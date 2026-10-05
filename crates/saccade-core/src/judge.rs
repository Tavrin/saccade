//! Judge mode: bounded technical and quality judgements made by a panel of
//! decision models and humans, with an honest account of how far each answer
//! can be trusted.
//!
//! A run builds *items* (a decision-request question about a report entry, or
//! a pairwise preference between two images), mixes in gold canaries, asks
//! every judge of a [`Panel`] in one or both presentation orders, measures
//! position bias, aggregates by weight with agreement metrics and escalates
//! disagreement to a person instead of guessing. Results are recorded as
//! proposals through the existing `decide` path. No panel answer becomes a
//! final decision or grants approval authority.
//!
//! The statistics live in [`crate::judge_stats`], the evidence encoder in
//! [`crate::judge_evidence`], the providers in [`crate::judge_provider`], the
//! canaries in [`crate::judge_canary`] and the human vote store in
//! [`crate::judge_vote`].

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use image::RgbImage;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::compare::{CompareOptions, compare_rgba};
use crate::decision::{
    Answer, DecideError, DecisionsConfig, Question, RequestOptions, build_request,
};
use crate::error::Error;
use crate::hotspots::{HotspotOptions, find_hotspots};
use crate::judge_canary::{Canary, canary_count, gold_set, score};
use crate::judge_evidence::{
    Extras, colour_shifts, flip_grid, ocr_diff, png_bytes, strips as make_strips,
};
use crate::judge_provider::{AskRequest, Attempt, Backend, CallOutcome, Prompt};
use crate::judge_stats::{Bias, PairObs, bradley_terry, is_abstain, position_bias};
use crate::rank::RankReport;
use crate::report::{Entry, Hotspot, Report};

/// Schema identifier of a judge result.
pub const JUDGE_SCHEMA: &str = "saccade-judge.v1";
/// Schema identifier of a self-test result.
pub const SELFTEST_SCHEMA: &str = "saccade-judge-selftest.v1";
/// File name of a judge result next to the report it judged.
pub const JUDGE_FILE_NAME: &str = "saccade-judge.v1.json";
/// Source name under which the aggregated panel answer is recorded.
pub const PANEL_SOURCE: &str = "panel";

/// Most hotspot strips a vision judge sees per call.
const VISION_STRIPS: usize = 2;

/// What kind of truth a question has. The kind decides how its answers may be
/// read; every judge output states it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// An objective truth exists ("is the shadow missing?").
    Checkable,
    /// Experts apply a shared standard ("acceptable per our art direction").
    Rubric,
    /// The answer depends on who is asked ("which cover is more appealing").
    Preference,
}

impl Kind {
    /// The name used in JSON and prompts.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Checkable => "checkable",
            Self::Rubric => "rubric",
            Self::Preference => "preference",
        }
    }

    /// What this kind of answer can and cannot tell you.
    pub fn limits(self) -> &'static str {
        match self {
            Self::Checkable => {
                "an objective truth exists, so a wrong answer is an error and accuracy can be measured against labelled items; a judge that is not measured is still unproven"
            }
            Self::Rubric => {
                "the answer applies a standard that experts share only partly; a judge is at best as consistent as the people who wrote and apply the rubric, so measure agreement with humans and expect a human-human ceiling below 1"
            }
            Self::Preference => {
                "the answer depends on who is asked; there is no single truth to be right about, so the result is the vote of this panel, never a fact about the images"
            }
        }
    }
}

/// A question a judge can be asked: one of the decision questions, or a
/// pairwise preference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JudgeQuestion {
    /// A `saccade decision-request` question.
    Decision(Question),
    /// Which of two images is preferred by a stated criterion.
    Preference,
}

impl JudgeQuestion {
    /// Every question name, in documentation order.
    pub const NAMES: [&'static str; 6] = [
        "accept",
        "triage",
        "cause",
        "ask_human",
        "mask_suggest",
        "preference",
    ];

    /// Parses a question name.
    pub fn parse(s: &str) -> Option<Self> {
        if s == "preference" {
            return Some(Self::Preference);
        }
        Question::parse(s).map(Self::Decision)
    }

    /// The name used on the command line and in JSON.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Decision(q) => q.as_str(),
            Self::Preference => "preference",
        }
    }

    /// The fixed wording.
    pub fn text(self) -> &'static str {
        match self {
            Self::Decision(q) => q.text(),
            Self::Preference => {
                "Which of the two images is better by the stated criterion? P1 is the first image, P2 the second."
            }
        }
    }

    /// The truth status of the question.
    pub fn kind(self) -> Kind {
        match self {
            Self::Decision(Question::Triage | Question::Cause) => Kind::Checkable,
            Self::Decision(_) => Kind::Rubric,
            Self::Preference => Kind::Preference,
        }
    }

    /// The answers a judge may give on the wire, an abstain option always included.
    pub fn wire_answers(self) -> Vec<String> {
        match self {
            Self::Preference => ["P1", "P2", "tie", "unsure"].map(str::to_owned).to_vec(),
            Self::Decision(q) => {
                let mut v: Vec<String> = q
                    .allowed_answers()
                    .iter()
                    .map(|s| (*s).to_owned())
                    .collect();
                if !v.iter().any(|a| is_abstain(a)) {
                    v.push("unsure".into());
                }
                v
            }
        }
    }
}

/// Which side is presented first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Order {
    /// The reference (or first candidate) is `P1`.
    Ab,
    /// The reference (or first candidate) is `P2`.
    Ba,
}

impl Order {
    fn as_str(self) -> &'static str {
        match self {
            Self::Ab => "ab",
            Self::Ba => "ba",
        }
    }

    fn idx(self) -> usize {
        match self {
            Self::Ab => 0,
            Self::Ba => 1,
        }
    }
}

/// The kinds of judge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Provider {
    /// TypeSafe Jev (text only, verified API).
    Jev,
    /// Google Gemini (`generateContent`, text and strips).
    Gemini,
    /// The `opencode run -m <model>` CLI.
    Opencode,
    /// Any OpenAI-compatible chat-completions endpoint.
    OpenaiCompatible,
    /// People voting through `saccade serve`.
    Human,
}

impl Provider {
    /// The name used in TOML and JSON.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Jev => "jev",
            Self::Gemini => "gemini",
            Self::Opencode => "opencode",
            Self::OpenaiCompatible => "openai_compatible",
            Self::Human => "human",
        }
    }
}

fn one() -> f64 {
    1.0
}

fn default_timeout() -> u32 {
    120
}

/// One `[[judge]]` of a panel file.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JudgeSpec {
    /// Name in results and as the proposal source; defaults to the provider.
    #[serde(default)]
    pub id: String,
    /// Which provider answers.
    pub provider: Provider,
    /// The pinned model (never a `-latest` alias, except Jev's own).
    #[serde(default)]
    pub model: String,
    /// What the judge is told it is.
    #[serde(default)]
    pub role: String,
    /// The standard it judges by.
    #[serde(default)]
    pub rubric: String,
    /// Version label of the rubric; a hash of the text when omitted.
    #[serde(default)]
    pub rubric_version: Option<String>,
    /// Weight in the aggregate.
    #[serde(default = "one")]
    pub weight: f64,
    /// Questions this judge answers; all when empty.
    #[serde(default)]
    pub questions: Vec<String>,
    /// Models to try, in order, when the main one is unavailable.
    #[serde(default)]
    pub fallback: Vec<String>,
    /// Whether the judge also gets blind hotspot strips (never full frames).
    #[serde(default)]
    pub vision: bool,
    /// Endpoint root of an `openai_compatible` provider.
    #[serde(default)]
    pub base_url: Option<String>,
    /// File name, inside the keys directory, holding this provider's key.
    #[serde(default)]
    pub key_file: Option<String>,
    /// Variable name inside that file.
    #[serde(default)]
    pub key_var: Option<String>,
    /// Per-call timeout in seconds.
    #[serde(default = "default_timeout")]
    pub timeout_secs: u32,
}

impl JudgeSpec {
    pub(crate) fn answers(&self, q: JudgeQuestion) -> bool {
        self.questions.is_empty() || self.questions.iter().any(|n| n == q.as_str())
    }

    /// The rubric version recorded in the audit trail.
    pub fn rubric_version(&self) -> String {
        self.rubric_version.clone().unwrap_or_else(|| {
            let h = Sha256::digest(self.rubric.as_bytes());
            format!("sha256:{}", hex(&h[..4]))
        })
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn sha(text: &str) -> String {
    format!("sha256:{}", hex(&Sha256::digest(text.as_bytes())))
}

fn p_min() -> f64 {
    0.6
}
fn p_agree() -> f64 {
    0.67
}
fn p_judges() -> usize {
    1
}
fn p_canary() -> f64 {
    0.25
}
fn p_pass() -> f64 {
    0.75
}
fn p_retries() -> u32 {
    3
}

/// The optional `[panel]` table.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PanelSettings {
    /// Panel name, shown in results.
    #[serde(default)]
    pub name: String,
    /// Winning probability below which the panel escalates to a person.
    #[serde(default = "p_min")]
    pub min_prob: f64,
    /// Weighted agreement below which the panel escalates.
    #[serde(default = "p_agree")]
    pub min_agreement: f64,
    /// Fewest committed judges for an answer.
    #[serde(default = "p_judges")]
    pub min_judges: usize,
    /// Canaries mixed in, as a fraction of the real items (0 disables).
    #[serde(default = "p_canary")]
    pub canary_rate: f64,
    /// Accuracy a judge needs on canaries not to be flagged.
    #[serde(default = "p_pass")]
    pub canary_pass: f64,
    /// Retries per model on 429/5xx.
    #[serde(default = "p_retries")]
    pub max_retries: u32,
}

impl Default for PanelSettings {
    fn default() -> Self {
        Self {
            name: String::new(),
            min_prob: p_min(),
            min_agreement: p_agree(),
            min_judges: p_judges(),
            canary_rate: p_canary(),
            canary_pass: p_pass(),
            max_retries: p_retries(),
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPanel {
    #[serde(default)]
    panel: PanelSettings,
    #[serde(default)]
    judge: Vec<JudgeSpec>,
}

/// A panel: settings and judges.
#[derive(Debug, Clone)]
pub struct Panel {
    /// The `[panel]` settings.
    pub settings: PanelSettings,
    /// The judges, ids filled in and unique.
    pub judges: Vec<JudgeSpec>,
}

fn slug(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect()
}

impl Panel {
    /// Parses a panel from TOML: an optional `[panel]` table and `[[judge]]` entries.
    pub fn parse(text: &str) -> Result<Self, Error> {
        let raw: RawPanel =
            toml::from_str(text).map_err(|e| Error::Config(format!("panel: {e}")))?;
        let mut judges = raw.judge;
        if judges.is_empty() {
            return Err(Error::Config("panel: add at least one [[judge]]".into()));
        }
        for j in &mut judges {
            if j.provider == Provider::Jev && j.vision {
                return Err(Error::Config(
                    "panel: Jev is text-only; vision must be false".into(),
                ));
            }
            if !(j.weight.is_finite() && j.weight > 0.0) {
                return Err(Error::Config(format!(
                    "panel: judge weight {} must be finite and above 0",
                    j.weight
                )));
            }
            if j.model.is_empty() {
                j.model = match j.provider {
                    Provider::Jev => "jev-latest".into(),
                    Provider::Gemini => "gemini-3.8-flash".into(),
                    Provider::Human => String::new(),
                    Provider::Opencode | Provider::OpenaiCompatible => {
                        return Err(Error::Config(format!(
                            "panel: a {} judge needs a `model`",
                            j.provider.as_str()
                        )));
                    }
                };
            }
            if j.provider != Provider::Jev {
                for m in std::iter::once(&j.model).chain(&j.fallback) {
                    if m.ends_with("-latest") || m == "latest" {
                        return Err(Error::Config(format!(
                            "panel: model {m:?} is a moving alias; pin an exact model"
                        )));
                    }
                }
            }
            if j.provider == Provider::Gemini
                && std::iter::once(&j.model).chain(&j.fallback).any(|m| {
                    !m.bytes()
                        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
                })
            {
                return Err(Error::Config(
                    "panel: unsafe Gemini model identifier".into(),
                ));
            }
            if j.provider == Provider::OpenaiCompatible && j.base_url.is_none() {
                return Err(Error::Config(
                    "panel: an openai_compatible judge needs `base_url`".into(),
                ));
            }
            if let Some(f) = &j.key_file
                && (f.contains(['/', '\\']) || f.starts_with('.'))
            {
                return Err(Error::Config(format!(
                    "panel: key_file {f:?} must be a plain file name inside the keys directory"
                )));
            }
            for q in &j.questions {
                if JudgeQuestion::parse(q).is_none() {
                    return Err(Error::Config(format!(
                        "panel: unknown question {q:?}; one of: {}",
                        JudgeQuestion::NAMES.join(", ")
                    )));
                }
            }
        }
        let mut seen: Vec<String> = Vec::new();
        for j in &mut judges {
            if j.id.is_empty() {
                j.id = j.provider.as_str().to_owned();
                if seen.contains(&j.id) {
                    j.id = format!("{}-{}", j.provider.as_str(), slug(&j.model));
                }
            }
            if j.id.to_ascii_lowercase().starts_with("human") && j.provider != Provider::Human {
                return Err(Error::Config(
                    "panel: only a human judge may have an id starting with `human`".into(),
                ));
            }
            if j.id == PANEL_SOURCE {
                return Err(Error::Config(
                    "panel: `panel` is a reserved judge id".into(),
                ));
            }
            if seen.contains(&j.id) {
                return Err(Error::Config(format!(
                    "panel: duplicate judge id {:?}",
                    j.id
                )));
            }
            seen.push(j.id.clone());
        }
        let s = &raw.panel;
        if !(0.0..=1.0).contains(&s.min_prob)
            || !(0.0..=1.0).contains(&s.min_agreement)
            || !(0.0..=1.0).contains(&s.canary_rate)
            || !(0.0..=1.0).contains(&s.canary_pass)
            || s.min_judges == 0
        {
            return Err(Error::Config(
                "panel: min_prob, min_agreement and canary_rate are fractions in [0, 1]".into(),
            ));
        }
        Ok(Self {
            settings: raw.panel,
            judges,
        })
    }

    /// Reads a panel file.
    pub fn from_file(path: &Path) -> Result<Self, Error> {
        let text = std::fs::read_to_string(path).map_err(|source| Error::Io {
            context: format!("reading panel {}", path.display()),
            source,
        })?;
        Self::parse(&text)
    }
}

// ---- items ---------------------------------------------------------------------

/// Where an item's pixels come from, for strips and the human vote page.
#[derive(Clone)]
pub enum PixelSource {
    /// Two image files, loaded when needed.
    Files(PathBuf, PathBuf),
    /// Generated images (canaries).
    Mem(Arc<(RgbImage, RgbImage)>),
}

impl PixelSource {
    pub(crate) fn load(&self) -> Option<(RgbImage, RgbImage)> {
        match self {
            Self::Mem(m) => Some((m.0.clone(), m.1.clone())),
            Self::Files(a, b) => {
                let open = |p: &Path| {
                    image::open(p)
                        .ok()
                        .map(|i| crate::compare::flatten_over(&i.to_rgba8(), 0))
                };
                Some((open(a)?, open(b)?))
            }
        }
    }
}

/// A canary's identity inside a run.
#[derive(Debug, Clone)]
pub struct CanaryInfo {
    /// What the pair is.
    pub label: &'static str,
    /// The correct answer.
    pub truth: &'static str,
}

/// One question about one subject, with its evidence in both orders.
#[derive(Clone)]
pub struct JudgeItem {
    /// Stable id inside the run.
    pub id: String,
    /// The entry (image name) it is about.
    pub entry: String,
    /// For `mask_suggest`: the 1-based hotspot.
    pub hotspot: Option<u32>,
    /// The question.
    pub question: JudgeQuestion,
    /// The decision-request `request_hash` of what was asked.
    pub request_hash: String,
    /// The evidence for the A/B and the B/A presentation.
    pub states: [Value; 2],
    /// Pixels for strips and the vote page.
    pub source: Option<PixelSource>,
    /// Hotspots of the compared pair, for strips.
    pub hotspots: Vec<Hotspot>,
    /// Set for a canary.
    pub canary: Option<CanaryInfo>,
    /// For a ranking: the candidates `(first, second)` compared.
    pub pair: Option<(usize, usize)>,
    /// The intent or preference criterion.
    pub intent: Option<String>,
}

/// Options of the evidence encoder.
#[derive(Debug, Clone, Default)]
pub struct EvidenceOptions {
    /// External OCR command (`--ocr-cmd`); `{}` stands for the image path.
    pub ocr_cmd: Option<String>,
}

fn synthetic_report(entry: Entry, ppd: f32) -> Report {
    use crate::report::{Labels, MetaSettings, Mode, REPORT_SCHEMA, ReportConfig, Totals};
    Report {
        perf_diff: None,
        perf_errors: Vec::new(),
        combined_verdict: None,
        exclusion_audit: None,
        schema: REPORT_SCHEMA.into(),
        tool_version: env!("CARGO_PKG_VERSION").into(),
        generated_at_unix: 0,
        baseline_dir: None,
        capture_dir: None,
        config: ReportConfig {
            mask_mode: crate::compare::MaskMode::default(),
            entries: Vec::new(),
            ignore: Vec::new(),
            default_threshold: 0.01,
            default_metric: crate::report::Metric::Mean,
            pixels_per_degree: ppd,
            fail_on_new: false,
            mode: Mode::Regression,
            labels: Labels::default(),
            meta: MetaSettings::default(),
            allow_empty: false,
            fail_on_nonfinite: true,
            hotspot_fail: None,
            hotspot_local_max: 0.5,
            hotspot_local_min_pixels: 16,
        },
        totals: Totals::default(),
        entries: vec![entry],
    }
}

/// Compares two images locally and returns a synthetic report entry for them.
fn compute_entry(
    name: &str,
    first: &RgbImage,
    second: &RgbImage,
    ppd: f32,
) -> Result<(Entry, Vec<f32>), Error> {
    use crate::diagnostics::{DiagnoseRequest, DiagnosticsConfig, Pixels, diagnose};
    let (a, b) = (
        image::DynamicImage::ImageRgb8(first.clone()).to_rgba8(),
        image::DynamicImage::ImageRgb8(second.clone()).to_rgba8(),
    );
    let opts = CompareOptions {
        pixels_per_degree: ppd,
        ..CompareOptions::default()
    };
    let cmp = compare_rgba(&b, &a, &opts)?;
    let (w, h) = first.dimensions();
    let hot_opts = HotspotOptions::default();
    let hotspots = find_hotspots(&cmp.error_map, None, w, h, &hot_opts);
    let identical = first.as_raw() == second.as_raw();
    let diag = diagnose(&DiagnoseRequest {
        baseline: Pixels::Ldr(&a),
        capture: Pixels::Ldr(&b),
        comparison: &cmp,
        flip: &opts,
        bit_identical: Some(identical),
        baseline_properties: Some(crate::properties::validate(first)),
        capture_properties: Some(crate::properties::validate(second)),
        hotspots: &hotspots,
        hotspot_options: hot_opts,
        config: &DiagnosticsConfig::default(),
        capture_path: None,
        out: None,
    })?;
    let value = cmp.metrics.mean;
    let entry = Entry {
        intended_variables: Vec::new(),
        required_effects: Vec::new(),
        buffer: None,
        name: name.to_owned(),
        status: if value > 0.01 {
            crate::report::Status::Fail
        } else {
            crate::report::Status::Pass
        },
        metric_used: crate::report::Metric::Mean,
        threshold: 0.01,
        value: Some(value),
        metrics: Some(cmp.metrics),
        properties: Some(crate::properties::validate(second)),
        paths: crate::report::EntryPaths::default(),
        error: None,
        regions: Vec::new(),
        masked_fraction: None,
        pixel_exclusions: None,
        sample_exclusions: None,
        bit_identical: Some(identical),
        hdr: None,
        meta_diff: Vec::new(),
        meta_ignored_diff: Vec::new(),
        file_bytes_identical: None,
        capture_validity: Default::default(),
        capture_provenance: Default::default(),
        meta_declared_unchanged: Vec::new(),
        baseline_properties: Some(crate::properties::validate(first)),
        warnings: Vec::new(),
        baseline_sha256: None,
        capture_sha256: None,
        hotspots,
        changed_pixel_runs: Vec::new(),
        object_attribution: Vec::new(),
        pass_with_local_change: false,
        diagnostics: Some(diag.diagnostics),
    };
    Ok((entry, cmp.error_map))
}

fn put_extras(state: &mut Value, extras: &Extras) {
    if let (Some(o), Ok(Value::Object(e))) = (state.as_object_mut(), serde_json::to_value(extras)) {
        o.extend(e);
    }
}

fn diagnostic_details(state: &mut Value, entry: &Entry) {
    if let Some(d) = &entry.diagnostics
        && let Some(o) = state["diagnostics"].as_object_mut()
    {
        o.insert("tone".into(), json!(d.tone));
        if let Some(s) = &d.shift {
            o.insert(
                "shift".into(),
                json!({"dx": s.dx, "dy": s.dy,
                    "confidence": s.confidence, "detected": s.detected,
                    "explained_fraction": s.shift_explained_fraction}),
            );
        }
    }
    state["metadata_differences"] = json!(entry.meta_diff);
}

/// The evidence state of a pair compared locally, in the presented order.
fn pair_state(
    name: &str,
    first: &RgbImage,
    second: &RgbImage,
    ppd: f32,
    intent: Option<&str>,
) -> Result<(Value, Vec<Hotspot>), Error> {
    let (entry, map) = compute_entry(name, first, second, ppd)?;
    let report = synthetic_report(entry.clone(), ppd);
    let mut state = crate::decision::entry_state(&report, &entry, intent);
    diagnostic_details(&mut state, &entry);
    let (w, h) = first.dimensions();
    let extras = Extras {
        flip_grid_8x8: flip_grid(&map, w, h),
        colour_shifts: colour_shifts(first, second, &map, &entry.hotspots, 3),
        ocr_diff: None,
    };
    put_extras(&mut state, &extras);
    if let Some(o) = state.as_object_mut() {
        o.insert("image_size".into(), json!([w, h]));
    }
    Ok((state, entry.hotspots))
}

fn load_rgb(path: &Path) -> Result<RgbImage, Error> {
    let img = image::open(path).map_err(|source| Error::Decode {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(crate::compare::flatten_over(&img.to_rgba8(), 0))
}

fn safe_join(dir: &Path, rel: &str) -> Result<PathBuf, Error> {
    if !crate::view::is_safe_name(rel) {
        return Err(Error::Config(format!(
            "unsafe image path {rel:?} in the report"
        )));
    }
    Ok(dir.join(rel))
}

/// The items for a report: one per failing entry and question, built from the
/// decision request (hashes included) plus the pixel evidence of each entry.
///
/// `skipped` lists the entries a model may not answer (deterministic failures).
pub fn report_items(
    report: &Report,
    report_dir: &Path,
    question: JudgeQuestion,
    intent: Option<&str>,
    entries: &[String],
    ev: &EvidenceOptions,
) -> Result<(Vec<JudgeItem>, Vec<Value>), DecideError> {
    let q = match question {
        JudgeQuestion::Decision(q) => q,
        JudgeQuestion::Preference => Question::Accept,
    };
    let req = build_request(
        report,
        &RequestOptions {
            question: Some(q),
            entries: entries.to_vec(),
            all_failing: entries.is_empty(),
            intent: intent.map(str::to_owned),
        },
    )?;
    let skipped = req["skipped"].as_array().cloned().unwrap_or_default();
    let mut items = Vec::new();
    let mut cache: BTreeMap<String, Option<PixelCache>> = BTreeMap::new();
    for it in req["items"].as_array().cloned().unwrap_or_default() {
        let name = it["entry"].as_str().unwrap_or_default().to_owned();
        let Some(entry) = report.entries.iter().find(|e| e.name == name) else {
            continue;
        };
        let px = cache
            .entry(name.clone())
            .or_insert_with(|| pixel_cache(report, report_dir, entry, ev).ok())
            .clone();
        let hotspot = it["hotspot"].as_u64().map(|n| n as u32);
        let mut state = it["state"].clone();
        diagnostic_details(&mut state, entry);
        let (source, hotspots) = match &px {
            Some(p) => {
                put_extras(&mut state, &p.extras);
                (Some(p.source.clone()), entry.hotspots.clone())
            }
            None => {
                if let Some(o) = state.as_object_mut() {
                    o.insert(
                        "evidence_note".into(),
                        json!("pixel evidence unavailable: the report's images could not be read"),
                    );
                }
                (None, entry.hotspots.clone())
            }
        };
        let id = it["id"].as_str().unwrap_or(&name).to_owned();
        let mut states = [state.clone(), state];
        if question == JudgeQuestion::Preference {
            // Preference: the first image is the baseline in A/B order, the capture in B/A.
            let hash = it["request_hash"].as_str().unwrap_or_default().to_owned();
            if let Some(p) = &px
                && let Some((a, b)) = p.source.load()
            {
                let ppd = report.config.pixels_per_degree;
                let ab = pair_state(&name, &a, &b, ppd, intent).map_err(DecideError::from)?;
                let ba = pair_state(&name, &b, &a, ppd, intent).map_err(DecideError::from)?;
                states = [ab.0, ba.0];
            }
            items.push(JudgeItem {
                id,
                entry: name,
                hotspot: None,
                question,
                request_hash: hash,
                states,
                source,
                hotspots,
                canary: None,
                pair: None,
                intent: intent.map(str::to_owned),
            });
            continue;
        }
        items.push(JudgeItem {
            id,
            entry: name,
            hotspot,
            question,
            request_hash: it["request_hash"].as_str().unwrap_or_default().to_owned(),
            states,
            source,
            hotspots,
            canary: None,
            pair: None,
            intent: intent.map(str::to_owned),
        });
    }
    Ok((items, skipped))
}

#[derive(Clone)]
struct PixelCache {
    extras: Extras,
    source: PixelSource,
}

fn pixel_cache(
    report: &Report,
    dir: &Path,
    entry: &Entry,
    ev: &EvidenceOptions,
) -> Result<PixelCache, Error> {
    let (Some(b), Some(c)) = (&entry.paths.baseline, &entry.paths.capture) else {
        return Err(Error::Config("the entry has no image copies".into()));
    };
    let (bp, cp) = (safe_join(dir, b)?, safe_join(dir, c)?);
    let (base, cap) = (load_rgb(&bp)?, load_rgb(&cp)?);
    let (w, h) = base.dimensions();
    let opts = CompareOptions {
        pixels_per_degree: report.config.pixels_per_degree,
        ..CompareOptions::default()
    };
    let cmp = compare_rgba(
        &image::DynamicImage::ImageRgb8(cap.clone()).to_rgba8(),
        &image::DynamicImage::ImageRgb8(base.clone()).to_rgba8(),
        &opts,
    )?;
    let ocr = ev
        .ocr_cmd
        .as_deref()
        .and_then(|cmd| ocr_diff(cmd, &bp, &cp).ok());
    Ok(PixelCache {
        extras: Extras {
            flip_grid_8x8: flip_grid(&cmp.error_map, w, h),
            colour_shifts: colour_shifts(&base, &cap, &cmp.error_map, &entry.hotspots, 3),
            ocr_diff: ocr,
        },
        source: PixelSource::Files(bp, cp),
    })
}

/// Pairwise preference items between the candidates of a ranking, `max_items`
/// of them, spread evenly over candidate pairs and images.
pub fn rank_items(
    rank: &RankReport,
    criterion: Option<&str>,
    max_items: usize,
) -> Result<(Vec<JudgeItem>, Vec<String>), Error> {
    let mut reports: Vec<(String, Report, PathBuf)> = Vec::new();
    for c in &rank.overall {
        let path = PathBuf::from(&c.report_json);
        let text = std::fs::read_to_string(&path).map_err(|source| Error::Io {
            context: format!("reading {}", path.display()),
            source,
        })?;
        let r: Report = serde_json::from_str(&text)?;
        let dir = path
            .parent()
            .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
        reports.push((c.label.clone(), r, dir));
    }
    let labels: Vec<String> = reports.iter().map(|r| r.0.clone()).collect();
    let mut names: Vec<String> = reports
        .first()
        .map(|r| r.1.entries.iter().map(|e| e.name.clone()).collect())
        .unwrap_or_default();
    names.retain(|n| {
        reports.iter().all(|(_, r, _)| {
            r.entries
                .iter()
                .any(|e| &e.name == n && e.paths.capture.is_some() && e.metrics.is_some())
        })
    });
    let mut pairs = Vec::new();
    for i in 0..reports.len() {
        for j in i + 1..reports.len() {
            pairs.push((i, j));
        }
    }
    let mut items = Vec::new();
    if pairs.is_empty() || names.is_empty() {
        return Ok((items, labels));
    }
    for k in 0..max_items.min(pairs.len() * names.len()) {
        let (i, j) = pairs[k % pairs.len()];
        let name = &names[(k / pairs.len()) % names.len()];
        let path_of = |idx: usize| -> Result<PathBuf, Error> {
            let (_, r, dir) = &reports[idx];
            let e = r.entries.iter().find(|e| &e.name == name);
            let rel = e
                .and_then(|e| e.paths.capture.as_deref())
                .unwrap_or_default();
            safe_join(dir, rel)
        };
        let (pa, pb) = (path_of(i)?, path_of(j)?);
        let (a, b) = (load_rgb(&pa)?, load_rgb(&pb)?);
        if a.dimensions() != b.dimensions() {
            continue;
        }
        let ppd = reports[i].1.config.pixels_per_degree;
        let mut ab = pair_state(name, &a, &b, ppd, criterion)?;
        let mut ba = pair_state(name, &b, &a, ppd, criterion)?;
        let value = |idx: usize| {
            reports[idx]
                .1
                .entries
                .iter()
                .find(|e| &e.name == name)
                .and_then(|e| e.value)
        };
        let dist = |first: usize, second: usize| {
            json!({"metric": format!("{:?}", rank.metric).to_lowercase(),
                   "P1": value(first), "P2": value(second)})
        };
        for (state, d) in [(&mut ab.0, dist(i, j)), (&mut ba.0, dist(j, i))] {
            if let Some(o) = state.as_object_mut() {
                o.insert("distance_to_common_reference".into(), d);
            }
        }
        let id = format!("{}~{}:{}", labels[i], labels[j], name);
        let hash = sha(&format!("preference|{id}|{}", criterion.unwrap_or("")));
        items.push(JudgeItem {
            id,
            entry: name.clone(),
            hotspot: None,
            question: JudgeQuestion::Preference,
            request_hash: hash,
            states: [ab.0, ba.0],
            source: Some(PixelSource::Mem(Arc::new((a, b)))),
            hotspots: ab.1,
            canary: None,
            pair: Some((i, j)),
            intent: criterion.map(str::to_owned),
        });
    }
    Ok((items, labels))
}

/// The gold canaries as items. Each is evidence built through the same
/// encoder as a real item, so a judge cannot tell it apart.
pub fn canary_items(n: usize) -> Vec<JudgeItem> {
    let mut out = Vec::new();
    for c in gold_set().into_iter().take(n) {
        let Canary {
            label,
            name,
            question,
            truth,
            intent,
            baseline,
            capture,
        } = c;
        let Ok((mut state, hotspots)) = pair_state(&name, &baseline, &capture, 67.0, intent) else {
            continue;
        };
        if let Some(o) = state.as_object_mut() {
            o.insert("verdict".into(), json!("fail"));
        }
        let hash = sha(&format!("canary|{name}|{}", question.as_str()));
        out.push(JudgeItem {
            id: name.clone(),
            entry: name,
            hotspot: None,
            question: JudgeQuestion::Decision(question),
            request_hash: hash,
            states: [state.clone(), state],
            source: Some(PixelSource::Mem(Arc::new((baseline, capture)))),
            hotspots,
            canary: Some(CanaryInfo { label, truth }),
            pair: None,
            intent: intent.map(str::to_owned),
        });
    }
    out
}

// ---- prompts -----------------------------------------------------------------------

fn sides(item: &JudgeItem, order: Order) -> (Value, String) {
    let (first_is_ref, label) = match order {
        Order::Ab => (true, "P1"),
        Order::Ba => (false, "P2"),
    };
    if item.question == JudgeQuestion::Preference {
        let criterion = item.intent.as_deref().unwrap_or("overall visual quality");
        return (
            json!({"P1": "first image", "P2": "second image", "criterion": criterion}),
            format!(
                "P1 and P2 are two candidates and neither is a reference. Decide by this criterion: {criterion}. Answer P1, P2, tie or unsure."
            ),
        );
    }
    let other = if first_is_ref { "P2" } else { "P1" };
    (
        json!({"reference": label, "candidate": other}),
        format!(
            "P1 and P2 are two renderings of the same frame. {label} is the REFERENCE (the approved baseline) and {other} is the CANDIDATE (the new capture). In the evidence, `reference` and `candidate` name those roles."
        ),
    )
}

/// The evidence of `item` for `order`, with the sides labelled.
pub fn labelled_state(item: &JudgeItem, order: Order) -> Value {
    let mut st = item.states[order.idx()].clone();
    if let Some(o) = st.as_object_mut() {
        o.insert("sides".into(), sides(item, order).0);
    }
    st
}

/// The prompt for one judge, item and order. `strips` is the number of
/// images that will be attached.
pub fn build_prompt(
    spec: &JudgeSpec,
    item: &JudgeItem,
    order: Order,
    wire: &[String],
    strips: usize,
) -> (Prompt, Value) {
    let state = labelled_state(item, order);
    let role = if spec.role.is_empty() {
        "a careful image-comparison judge"
    } else {
        &spec.role
    };
    let mut system = format!(
        "You are {role}. You answer one bounded question about a pair of images from evidence. Answer only with one of the allowed answers. When the evidence does not settle the question, answer \"unsure\"; never guess to be helpful."
    );
    if !spec.rubric.is_empty() {
        system.push_str(&format!(
            "\n\nRubric (version {}):\n{}",
            spec.rubric_version(),
            spec.rubric
        ));
    }
    let kind = item.question.kind();
    let mut user = format!(
        "Question kind: {} ({}).\nQuestion: {}\nAllowed answers: {}\n{}\n",
        kind.as_str(),
        kind.limits(),
        item.question.text(),
        wire.join(", "),
        sides(item, order).1,
    );
    if strips > 0 {
        user.push_str(&format!(
            "Attached: {strips} blind image strip(s); each shows crops of the changed region, image 1 on the left and image 2 on the right, contrast-stretched when dark. Image 1 is {}.\n",
            "P1",
        ));
    }
    user.push_str(&format!(
        "Evidence (JSON, no pixels are included):\n{}\n\nReply with exactly one JSON object: {{\"answer\": \"<one allowed answer>\", \"probability\": <number from 0 to 1, how likely your answer is correct>}}",
        state
    ));
    (Prompt { system, user }, state)
}

pub(crate) fn strips_for(item: &JudgeItem, order: Order) -> Vec<Vec<u8>> {
    let Some((a, b)) = item.source.as_ref().and_then(PixelSource::load) else {
        return Vec::new();
    };
    let swap = order == Order::Ba;
    make_strips(&a, &b, &item.hotspots, VISION_STRIPS, swap)
        .iter()
        .map(png_bytes)
        .filter(|p| !p.is_empty())
        .collect()
}

fn strip_count(item: &JudgeItem) -> usize {
    if item.source.is_none() {
        0
    } else {
        item.hotspots.len().min(VISION_STRIPS)
    }
}

/// Maps a wire answer back to the stable answer space of the item.
fn stable_answer(item: &JudgeItem, order: Order, wire: &str) -> String {
    if item.question != JudgeQuestion::Preference {
        return wire.to_owned();
    }
    match (wire, order) {
        ("P1", Order::Ab) | ("P2", Order::Ba) => "a".into(),
        ("P2", Order::Ab) | ("P1", Order::Ba) => "b".into(),
        (w, _) => w.to_owned(),
    }
}

/// The stable answers of an item (abstain included).
fn stable_answers(item: &JudgeItem) -> Vec<String> {
    match item.question {
        JudgeQuestion::Preference => ["a", "b", "tie", "unsure"].map(str::to_owned).to_vec(),
        q => q.wire_answers(),
    }
}

// ---- judgements ------------------------------------------------------------------------

/// One call's outcome: the audit record of a single judgement.
#[derive(Debug, Clone, Serialize)]
pub struct Judgement {
    /// The judge id (`human:<name>` for a voter).
    pub judge: String,
    /// The provider.
    pub provider: String,
    /// The model requested.
    pub model: String,
    /// The model that answered (differs after a fallback).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub answered_model: Option<String>,
    /// The version string the provider reported.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_version: Option<String>,
    /// `ab` or `ba`.
    pub order: &'static str,
    /// The item answered.
    pub item: String,
    /// The answer in the item's stable answer space; `None` when the call failed.
    pub answer: Option<String>,
    /// The answer as the judge gave it (`P1`/`P2` for preferences).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wire_answer: Option<String>,
    /// The probability the judge gives its answer.
    pub prob: Option<f64>,
    /// A second confidence figure, when reported.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f64>,
    /// `model_distribution`, `verbalized` or `human_vote`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prob_source: Option<&'static str>,
    /// The full distribution over stable answers, when reported.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub probs: BTreeMap<String, f64>,
    /// Whether this judgement abstains (an abstain answer or a failed call).
    pub abstained: bool,
    /// Why it abstains.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub abstain_reason: Option<String>,
    /// Wall time of the call.
    pub latency_ms: u64,
    /// Provider-reported token usage, when available.
    pub usage: Value,
    /// The models tried, with retries and statuses.
    pub attempts: Vec<Attempt>,
    /// Whether a fallback model answered.
    pub fallback_used: bool,
    /// The rubric version the judge was given.
    pub rubric_version: String,
    /// SHA-256 of the exact prompt and strips sent.
    pub evidence_hash: String,
    /// When it was recorded, Unix milliseconds.
    pub timestamp_ms: u64,
}

pub(crate) fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

/// A judge's committed view of one item, orders merged.
#[derive(Debug, Clone)]
pub struct AggVote {
    /// The judge id.
    pub judge: String,
    /// The effective weight (canary down-weighting applied).
    pub weight: f64,
    /// The merged answer; `None` abstains.
    pub answer: Option<String>,
    /// The distribution over the item's answers.
    pub probs: BTreeMap<String, f64>,
}

/// Thresholds of the aggregate.
#[derive(Debug, Clone, Copy)]
pub struct AggParams {
    /// Winning probability below which the panel escalates.
    pub min_prob: f64,
    /// Weighted agreement below which the panel escalates.
    pub min_agreement: f64,
    /// Fewest committed judges.
    pub min_judges: usize,
}

/// The panel's answer to one item.
#[derive(Debug, Clone, PartialEq)]
pub struct Aggregate {
    /// The winning answer, or `None` when the panel escalates.
    pub answer: Option<String>,
    /// Whether a person must look.
    pub escalated: bool,
    /// Weighted probability of the winner among committed judges.
    pub prob: f64,
    /// Weighted share of committed judges that chose the winner.
    pub agreement: f64,
    /// Judges that committed.
    pub answering: usize,
    /// Judges that abstained.
    pub abstained: usize,
    /// Why it escalated.
    pub reasons: Vec<String>,
}

/// Weighted aggregation with escalation. Never a coin flip: a tie, weak
/// support or too little agreement escalates.
pub fn aggregate(votes: &[AggVote], p: &AggParams) -> Aggregate {
    let committed: Vec<&AggVote> = votes
        .iter()
        .filter(|v| v.answer.as_deref().is_some_and(|a| !is_abstain(a)))
        .collect();
    let abstained = votes.len() - committed.len();
    let mut out = Aggregate {
        answer: None,
        escalated: true,
        prob: 0.0,
        agreement: 0.0,
        answering: committed.len(),
        abstained,
        reasons: Vec::new(),
    };
    let total: f64 = committed.iter().map(|v| v.weight).sum();
    if committed.is_empty() || total <= 0.0 {
        out.reasons.push("no judge committed to an answer".into());
        return out;
    }
    let mut score: BTreeMap<&str, f64> = BTreeMap::new();
    for v in &committed {
        for (a, pr) in &v.probs {
            if !is_abstain(a) {
                *score.entry(a).or_insert(0.0) += v.weight * pr;
            }
        }
    }
    let mut ranked: Vec<(&str, f64)> = score.into_iter().collect();
    ranked.sort_by(|a, b| b.1.total_cmp(&a.1));
    let Some(&(winner, top)) = ranked.first() else {
        out.reasons.push("no judge gave probabilities".into());
        return out;
    };
    if ranked.get(1).is_some_and(|s| (s.1 - top).abs() < 1e-12) {
        out.reasons.push("the top answers tie".into());
    }
    out.prob = top / total;
    let agree: f64 = committed
        .iter()
        .filter(|v| v.answer.as_deref() == Some(winner))
        .map(|v| v.weight)
        .sum();
    out.agreement = agree / total;
    if committed.len() < p.min_judges {
        out.reasons.push(format!(
            "only {} judge(s) committed; the panel needs {}",
            committed.len(),
            p.min_judges
        ));
    }
    if out.agreement < p.min_agreement {
        out.reasons.push(format!(
            "weighted agreement {:.2} is below {:.2}",
            out.agreement, p.min_agreement
        ));
    }
    if out.prob < p.min_prob {
        out.reasons.push(format!(
            "winning probability {:.2} is below {:.2}",
            out.prob, p.min_prob
        ));
    }
    if out.reasons.is_empty() {
        out.answer = Some(winner.to_owned());
        out.escalated = false;
    }
    out
}

/// The distribution of a judgement over the item's non-abstain answers.
fn dist_of(j: &Judgement, answers: &[String]) -> BTreeMap<String, f64> {
    if !j.probs.is_empty() {
        return j.probs.clone();
    }
    let Some(a) = &j.answer else {
        return BTreeMap::new();
    };
    let k = answers.iter().filter(|x| !is_abstain(x)).count().max(2);
    let p = j
        .prob
        .unwrap_or(if j.provider == "human" { 1.0 } else { 0.0 })
        .clamp(0.0, 1.0);
    answers
        .iter()
        .filter(|x| !is_abstain(x))
        .map(|x| {
            (
                x.clone(),
                if x == a {
                    p
                } else {
                    (1.0 - p) / (k as f64 - 1.0)
                },
            )
        })
        .collect()
}

/// One judge's orders merged into a vote, with how the orders agreed.
fn merge(js: &[&Judgement], answers: &[String], weight: f64) -> (AggVote, &'static str) {
    let judge = js.first().map_or_else(String::new, |j| j.judge.clone());
    let committed: Vec<&&Judgement> = js
        .iter()
        .filter(|j| j.answer.as_deref().is_some_and(|a| !is_abstain(a)))
        .collect();
    let mut vote = AggVote {
        judge,
        weight,
        answer: None,
        probs: BTreeMap::new(),
    };
    if committed.len() < js.len() {
        return (vote, "abstain");
    }
    let first = committed.first().and_then(|j| j.answer.clone());
    if committed.iter().any(|j| j.answer != first) {
        return (vote, "flip");
    }
    let mut sum: BTreeMap<String, f64> = BTreeMap::new();
    for j in &committed {
        for (a, p) in dist_of(j, answers) {
            *sum.entry(a).or_insert(0.0) += p / committed.len() as f64;
        }
    }
    vote.answer = first;
    vote.probs = sum;
    (
        vote,
        if js.len() > 1 {
            "consistent"
        } else {
            "single_order"
        },
    )
}

// ---- running ---------------------------------------------------------------------------

/// Options of a run.
#[derive(Debug, Clone)]
pub struct RunOptions {
    /// The question asked.
    pub question: JudgeQuestion,
    /// Ask text judges in both orders too (vision and human judges always are).
    pub both_orders: bool,
    /// Overrides the panel's canary rate.
    pub canary_rate: Option<f64>,
    /// Most calls a run may make.
    pub max_calls: usize,
    /// Where human votes live (`<dir>/judge/<run id>`); `None` skips human judges.
    pub decisions_dir: Option<PathBuf>,
    /// An optional calibration document (`saccade-calibration.v1`).
    pub calibration: Option<Value>,
}

/// The items of a run, canaries mixed in.
pub struct Plan {
    /// Real items followed by canaries (the asking order interleaves them).
    pub items: Vec<JudgeItem>,
    /// Entries a model may not answer.
    pub skipped: Vec<Value>,
    /// Deterministic id of the run: the same inputs give the same id.
    pub run_id: String,
}

/// Adds canaries to `items` and derives the run id.
pub fn make_plan(
    mut items: Vec<JudgeItem>,
    skipped: Vec<Value>,
    panel: &Panel,
    opts: &RunOptions,
) -> Plan {
    let real = items.len();
    let rate = opts.canary_rate.unwrap_or(panel.settings.canary_rate);
    let mut basis = format!(
        "{}|{}|{}",
        opts.question.as_str(),
        panel.settings.name,
        opts.both_orders
    );
    basis.push_str(&crate::labels::canonical_json(&panel_value(
        panel,
        &BTreeMap::new(),
    )));
    basis.push_str("|blind-vote-layout-v1.1|");
    for spec in &panel.judges {
        basis.push_str(&json!([spec.role, spec.rubric]).to_string());
    }
    for i in &items {
        basis.push('|');
        basis.push_str(&i.request_hash);
        basis.push_str(&i.id);
        basis.push_str(&crate::labels::canonical_json(&json!(i.states)));
        if let Some((a, b)) = i.source.as_ref().and_then(PixelSource::load) {
            basis.push_str(&hex(&Sha256::digest(a.as_raw())));
            basis.push_str(&hex(&Sha256::digest(b.as_raw())));
        }
    }
    let run_id = hex(&Sha256::digest(basis.as_bytes())[..8]);
    if opts.question != JudgeQuestion::Preference {
        let canaries = canary_items(canary_count(real, rate));
        // Spread the canaries through the run rather than appending them.
        let n = canaries.len();
        for (k, c) in canaries.into_iter().enumerate() {
            let at = ((k + 1) * (items.len() + 1) / (n + 1)).min(items.len());
            items.insert(at, c);
        }
    }
    Plan {
        items,
        skipped,
        run_id,
    }
}

fn judge_orders(spec: &JudgeSpec, item: &JudgeItem, both: bool) -> Vec<Order> {
    if both
        || spec.vision
        || spec.provider == Provider::Human
        || item.question == JudgeQuestion::Preference
    {
        vec![Order::Ab, Order::Ba]
    } else {
        vec![Order::Ab]
    }
}

fn applicable<'a>(panel: &'a Panel, item: &JudgeItem) -> Vec<&'a JudgeSpec> {
    panel
        .judges
        .iter()
        .filter(|j| j.provider != Provider::Human && j.answers(item.question))
        .collect()
}

/// The requests a run would make, without making any.
pub fn dry_run_value(plan: &Plan, panel: &Panel, opts: &RunOptions) -> Value {
    let mut requests = Vec::new();
    for item in &plan.items {
        for spec in applicable(panel, item) {
            let wire = item.question.wire_answers();
            for order in judge_orders(spec, item, opts.both_orders) {
                let n = if spec.vision { strip_count(item) } else { 0 };
                let (prompt, state) = build_prompt(spec, item, order, &wire, n);
                requests.push(json!({
                    "judge": spec.id,
                    "provider": spec.provider.as_str(),
                    "model": spec.model,
                    "fallback": spec.fallback,
                    "item": item.id,
                    "order": order.as_str(),
                    "question": item.question.as_str(),
                    "kind": item.question.kind().as_str(),
                    "allowed_answers": wire,
                    "vision_strips": n,
                    "canary": item.canary.is_some(),
                    "state": state,
                    "system": prompt.system,
                    "user": prompt.user,
                    "evidence_hash": sha(&format!("{}\n{}", prompt.system, prompt.user)),
                }));
            }
        }
    }
    let humans: Vec<Value> = panel
        .judges
        .iter()
        .filter(|j| j.provider == Provider::Human)
        .map(|j| {
            json!({"judge": j.id, "vote_path": format!("/vote/{}", plan.run_id),
            "items": plan.items.iter().filter(|i| i.canary.is_none()).count() * 2})
        })
        .collect();
    json!({
        "schema": JUDGE_SCHEMA,
        "dry_run": true,
        "run_id": plan.run_id,
        "question": question_value(opts.question),
        "panel": panel_value(panel, &BTreeMap::new()),
        "both_orders": opts.both_orders,
        "calls_planned": requests.len(),
        "requests": requests,
        "human_votes": humans,
        "skipped": plan.skipped,
    })
}

fn question_value(q: JudgeQuestion) -> Value {
    json!({
        "type": q.as_str(),
        "text": q.text(),
        "kind": q.kind().as_str(),
        "kind_limits": q.kind().limits(),
    })
}

fn panel_value(panel: &Panel, factors: &BTreeMap<String, (f64, bool)>) -> Value {
    json!({
        "name": panel.settings.name,
        "min_prob": panel.settings.min_prob,
        "min_agreement": panel.settings.min_agreement,
        "min_judges": panel.settings.min_judges,
        "judges": panel.judges.iter().map(|j| {
            let (f, flagged) = factors.get(&j.id).copied().unwrap_or((1.0, false));
            json!({
                "id": j.id, "provider": j.provider.as_str(), "model": j.model,
                "role": j.role, "rubric_version": j.rubric_version(),
                "weight": j.weight, "effective_weight": j.weight * f,
                "flagged_by_canaries": flagged,
                "vision": j.vision, "questions": j.questions, "fallback": j.fallback,
            })
        }).collect::<Vec<_>>(),
    })
}

/// A completed run: the result document and what to record.
pub struct RunResult {
    /// The `saccade-judge.v1` document.
    pub value: Value,
    /// Per real item: its id, entry, hotspot, question, request hash, each
    /// judge's merged answer and the panel's aggregate.
    pub outcomes: Vec<ItemOutcome>,
}

/// What a run decided about one real item.
pub struct ItemOutcome {
    /// The item.
    pub item: JudgeItem,
    /// Each judge's merged vote.
    pub votes: Vec<AggVote>,
    /// The panel's answer.
    pub aggregate: Aggregate,
    /// The rubric version and model of each judge, for proposal notes.
    pub notes: BTreeMap<String, String>,
}

pub(crate) fn judge_one(
    backend: &dyn Backend,
    spec: &JudgeSpec,
    item: &JudgeItem,
    order: Order,
    calls: &mut usize,
    max_calls: usize,
) -> Judgement {
    let wire = item.question.wire_answers();
    let images = if spec.vision {
        strips_for(item, order)
    } else {
        Vec::new()
    };
    let (prompt, state) = build_prompt(spec, item, order, &wire, images.len());
    let mut ev = Sha256::new();
    ev.update(prompt.system.as_bytes());
    ev.update(b"\n");
    ev.update(prompt.user.as_bytes());
    for i in &images {
        ev.update(Sha256::digest(i));
    }
    let evidence_hash = format!("sha256:{}", hex(&ev.finalize()));
    let mut j = Judgement {
        judge: spec.id.clone(),
        provider: spec.provider.as_str().to_owned(),
        model: spec.model.clone(),
        answered_model: None,
        model_version: None,
        order: order.as_str(),
        item: item.id.clone(),
        answer: None,
        wire_answer: None,
        prob: None,
        confidence: None,
        prob_source: None,
        probs: BTreeMap::new(),
        abstained: true,
        abstain_reason: None,
        latency_ms: 0,
        usage: Value::Null,
        attempts: Vec::new(),
        fallback_used: false,
        rubric_version: spec.rubric_version(),
        evidence_hash,
        timestamp_ms: now_ms(),
    };
    if *calls >= max_calls {
        j.abstain_reason = Some(format!("call budget of {max_calls} reached"));
        return j;
    }
    *calls += 1;
    let started = std::time::Instant::now();
    let out = backend.ask(&AskRequest {
        spec,
        question: item.question.as_str(),
        question_text: item.question.text(),
        kind: item.question.kind().as_str(),
        wire_answers: &wire,
        state: &state,
        prompt: &prompt,
        images: &images,
    });
    j.latency_ms = started.elapsed().as_millis() as u64;
    j.fallback_used = out.attempts.iter().any(|a| a.ok && a.model != spec.model);
    j.attempts = out.attempts;
    match out.result {
        Ok(raw) => {
            let stable = stable_answer(item, order, &raw.answer);
            j.abstained = is_abstain(&stable);
            if j.abstained {
                j.abstain_reason = Some("the judge answered that it is unsure".into());
            }
            j.wire_answer = Some(raw.answer);
            j.answered_model = Some(raw.model);
            j.model_version = Some(raw.model_version);
            j.prob = raw.prob;
            j.confidence = raw.confidence;
            j.prob_source = Some(raw.prob_source);
            j.latency_ms = j.latency_ms.max(raw.latency_ms);
            j.usage = raw.usage;
            j.probs = raw
                .probs
                .iter()
                .map(|(k, v)| (stable_answer(item, order, k), *v))
                .collect();
            j.answer = Some(stable);
            if j.prob.is_none()
                || j.prob
                    .is_some_and(|p| !p.is_finite() || !(0.0..=1.0).contains(&p))
            {
                j.abstained = true;
                j.answer = None;
                j.abstain_reason = Some("the judge did not report a valid probability".into());
            }
        }
        Err(e) => j.abstain_reason = Some(format!("provider unavailable: {e}")),
    }
    j
}

struct BatchReplay<'a> {
    backend: &'a dyn Backend,
    results: std::sync::Mutex<BTreeMap<String, CallOutcome>>,
}

impl Backend for BatchReplay<'_> {
    fn ask(&self, req: &AskRequest<'_>) -> CallOutcome {
        let key = format!("{}|{}", req.spec.id, req.prompt.user);
        if let Ok(mut results) = self.results.lock()
            && let Some(out) = results.remove(&key)
        {
            return out;
        }
        if req.spec.provider == Provider::Jev {
            return CallOutcome {
                result: Err("missing batched answer".into()),
                attempts: Vec::new(),
            };
        }
        self.backend.ask(req)
    }
}

/// Runs the panel over the plan with `backend` and returns the result.
pub fn execute(plan: &Plan, panel: &Panel, backend: &dyn Backend, opts: &RunOptions) -> RunResult {
    // Select exactly the logical calls allowed by the budget, then send each
    // Jev judge's indexed questions in a single batch. Both orders still have
    // independent answers and audit records.
    let mut batches: BTreeMap<&str, Vec<(&JudgeSpec, &JudgeItem, Order)>> = BTreeMap::new();
    let mut planned = 0;
    for item in &plan.items {
        for spec in applicable(panel, item) {
            for order in judge_orders(spec, item, opts.both_orders) {
                if planned < opts.max_calls && spec.provider == Provider::Jev {
                    batches
                        .entry(&spec.id)
                        .or_default()
                        .push((spec, item, order));
                }
                planned += 1;
            }
        }
    }
    let mut results = BTreeMap::new();
    for batch in batches.values() {
        let prepared: Vec<_> = batch
            .iter()
            .map(|(spec, item, order)| {
                let wire = item.question.wire_answers();
                let (prompt, state) = build_prompt(spec, item, *order, &wire, 0);
                (wire, prompt, state)
            })
            .collect();
        let requests: Vec<_> = batch
            .iter()
            .zip(&prepared)
            .map(|((spec, item, _), (wire, prompt, state))| AskRequest {
                spec,
                question: item.question.as_str(),
                question_text: item.question.text(),
                kind: item.question.kind().as_str(),
                wire_answers: wire,
                state,
                prompt,
                images: &[],
            })
            .collect();
        for (req, out) in requests.iter().zip(backend.ask_many(&requests)) {
            results.insert(format!("{}|{}", req.spec.id, req.prompt.user), out);
        }
    }
    let replay = BatchReplay {
        backend,
        results: std::sync::Mutex::new(results),
    };
    let mut judgements: Vec<Judgement> = Vec::new();
    let mut calls = 0usize;
    for item in &plan.items {
        for spec in applicable(panel, item) {
            for order in judge_orders(spec, item, opts.both_orders) {
                judgements.push(judge_one(
                    &replay,
                    spec,
                    item,
                    order,
                    &mut calls,
                    opts.max_calls,
                ));
            }
        }
    }
    // Human voters: each named voter is a judge with the weight of the `human` entry.
    let mut human_weights: BTreeMap<String, f64> = BTreeMap::new();
    let mut vote_info = Value::Null;
    if let (Some(dir), Some(h)) = (
        &opts.decisions_dir,
        panel.judges.iter().find(|j| j.provider == Provider::Human),
    ) {
        let (js, info) = crate::judge_vote::collect(dir, &plan.run_id, h, plan);
        for j in &js {
            human_weights.insert(j.judge.clone(), h.weight);
        }
        judgements.extend(js);
        vote_info = info;
    }
    // Canary scoring per judge, on merged answers.
    let mut factors: BTreeMap<String, (f64, bool)> = BTreeMap::new();
    let mut canary_rows = Vec::new();
    for spec in panel
        .judges
        .iter()
        .filter(|j| j.provider != Provider::Human)
    {
        let mut answers: Vec<(Option<String>, &'static str)> = Vec::new();
        for item in plan.items.iter().filter(|i| i.canary.is_some()) {
            let js: Vec<&Judgement> = judgements
                .iter()
                .filter(|j| j.judge == spec.id && j.item == item.id)
                .collect();
            if js.is_empty() {
                continue;
            }
            let (vote, _) = merge(&js, &stable_answers(item), 1.0);
            if let Some(c) = &item.canary {
                answers.push((vote.answer, c.truth));
            }
        }
        if answers.is_empty() {
            continue;
        }
        let pairs: Vec<(Option<&str>, &str)> =
            answers.iter().map(|(a, t)| (a.as_deref(), *t)).collect();
        let s = score(&pairs, panel.settings.canary_pass);
        factors.insert(spec.id.clone(), (s.weight_factor, s.flagged));
        canary_rows.push(json!({
            "judge": spec.id, "items": answers.len(), "correct": s.correct, "wrong": s.wrong,
            "abstained": s.abstained, "accuracy": s.accuracy, "flagged": s.flagged,
            "weight_factor": s.weight_factor,
        }));
    }
    let params = AggParams {
        min_prob: panel.settings.min_prob,
        min_agreement: panel.settings.min_agreement,
        min_judges: panel.settings.min_judges,
    };
    let weight_of = |judge: &str| -> f64 {
        if let Some(w) = human_weights.get(judge) {
            return *w;
        }
        panel
            .judges
            .iter()
            .find(|j| j.id == judge)
            .map_or(1.0, |j| j.weight * factors.get(judge).map_or(1.0, |f| f.0))
    };
    let mut outcomes = Vec::new();
    let mut item_values = Vec::new();
    for item in plan.items.iter().filter(|i| i.canary.is_none()) {
        let answers = stable_answers(item);
        let mut by_judge: BTreeMap<&str, Vec<&Judgement>> = BTreeMap::new();
        for j in judgements.iter().filter(|j| j.item == item.id) {
            by_judge.entry(&j.judge).or_default().push(j);
        }
        let mut votes = Vec::new();
        let mut per_judge = Vec::new();
        let mut notes = BTreeMap::new();
        for (judge, js) in &by_judge {
            let (mut vote, mut position) = merge(js, &answers, weight_of(judge));
            let both_required = js.first().is_some_and(|j| j.provider == "human")
                || panel
                    .judges
                    .iter()
                    .find(|s| s.id == *judge)
                    .is_some_and(|s| judge_orders(s, item, opts.both_orders).len() == 2);
            if both_required && js.len() < 2 {
                vote.answer = None;
                position = "incomplete_orders";
            }
            notes.insert(
                (*judge).to_owned(),
                format!(
                    "model={};rubric={};position={position}",
                    js.first()
                        .map_or("", |j| j.answered_model.as_deref().unwrap_or(&j.model)),
                    js.first().map_or("", |j| &j.rubric_version)
                ),
            );
            per_judge.push(json!({
                "judge": judge, "answer": vote.answer, "position": position,
                "effective_weight": vote.weight,
                "prob": vote.answer.as_ref().and_then(|a| vote.probs.get(a)),
            }));
            votes.push(vote);
        }
        let mut agg = aggregate(&votes, &params);
        if per_judge.iter().any(|v| v["position"] == "flip") {
            agg.answer = None;
            agg.escalated = true;
            agg.reasons
                .push("a judge changed its answer when the order was swapped".into());
        }
        let judgements_of_item: Vec<&Judgement> =
            judgements.iter().filter(|j| j.item == item.id).collect();
        item_values.push(json!({
            "id": item.id, "entry": item.entry, "hotspot": item.hotspot,
            "question": item.question.as_str(), "kind": item.question.kind().as_str(),
            "allowed_answers": stable_answers(item), "request_hash": item.request_hash,
            "judgements": judgements_of_item,
            "per_judge": per_judge,
            "result": result_value(item, &agg, &votes, panel, &opts.calibration),
        }));
        outcomes.push(ItemOutcome {
            item: item.clone(),
            votes,
            aggregate: agg,
            notes,
        });
    }
    // Position bias per judge, over every item answered in both orders.
    let mut bias_rows = Vec::new();
    let mut judge_ids: Vec<&str> = judgements.iter().map(|j| j.judge.as_str()).collect();
    judge_ids.sort_unstable();
    judge_ids.dedup();
    for id in judge_ids {
        let mut obs = Vec::new();
        for item in &plan.items {
            let slot = |o: &str| {
                judgements
                    .iter()
                    .find(|j| j.judge == id && j.item == item.id && j.order == o)
            };
            let (Some(ab), Some(ba)) = (slot("ab"), slot("ba")) else {
                continue;
            };
            let pick = |j: &Judgement| j.answer.clone().filter(|a| !is_abstain(a));
            obs.push(PairObs {
                ab: pick(ab),
                ba: pick(ba),
                ab_slot: ab
                    .wire_answer
                    .clone()
                    .filter(|_| item.question == JudgeQuestion::Preference),
                ba_slot: ba
                    .wire_answer
                    .clone()
                    .filter(|_| item.question == JudgeQuestion::Preference),
            });
        }
        if obs.is_empty() {
            continue;
        }
        let b: Bias = position_bias(&obs);
        let mut v = b.value();
        if let Some(o) = v.as_object_mut() {
            o.insert("judge".into(), json!(id));
        }
        bias_rows.push(v);
    }
    let ranking = ranking_value(plan, &outcomes);
    let value = json!({
        "schema": JUDGE_SCHEMA,
        "dry_run": false,
        "run_id": plan.run_id,
        "tool_version": env!("CARGO_PKG_VERSION"),
        "created_at_unix": now_ms() / 1000,
        "question": question_value(opts.question),
        "panel": panel_value(panel, &factors),
        "both_orders": opts.both_orders,
        "calls": calls,
        "items": item_values,
        "canaries": {"items": plan.items.iter().filter(|i| i.canary.is_some()).count(), "judges": canary_rows,
            "judgements": judgements.iter().filter(|j| plan.items.iter().any(|i| i.id == j.item && i.canary.is_some())).collect::<Vec<_>>()},
        "position_bias": bias_rows,
        "human_votes": vote_info,
        "ranking": ranking,
        "skipped": plan.skipped,
        "calibration": calibration_summary(&opts.calibration),
    });
    RunResult { value, outcomes }
}

fn calibration_summary(cal: &Option<Value>) -> Value {
    match cal {
        Some(c) => {
            json!({"applied": true, "labelled_units": c["labelled_units"], "human_ceiling": c["human_ceiling"]})
        }
        None => {
            json!({"applied": false, "note": "no calibration file: probabilities are the providers' own and have not been checked against human labels"})
        }
    }
}

fn result_value(
    item: &JudgeItem,
    agg: &Aggregate,
    votes: &[AggVote],
    panel: &Panel,
    cal: &Option<Value>,
) -> Value {
    let kind = item.question.kind();
    let answer = match (&agg.answer, item.question) {
        (Some(a), _) => json!(a),
        (None, JudgeQuestion::Decision(Question::Accept)) => json!("needs_human"),
        (None, _) => Value::Null,
    };
    let calibrated: Vec<String> = votes
        .iter()
        .filter_map(|v| {
            cal.as_ref()?["judges"]
                .as_array()?
                .iter()
                .find(|j| j["judge"] == v.judge.as_str() && j["question"] == item.question.as_str())
                .map(|j| format!("{} (ECE {}, n {})", v.judge, j["ece"], j["n"]))
        })
        .collect();
    let mut trust = format!("{} question: {}. ", kind.as_str(), kind.limits());
    if agg.escalated {
        trust.push_str(&format!(
            "The panel did not settle it ({}); a person must look. ",
            agg.reasons.join("; ")
        ));
    } else {
        trust.push_str(&format!(
            "{} of {} judge(s) committed, weighted agreement {:.2}, winning probability {:.2}. ",
            agg.answering,
            agg.answering + agg.abstained,
            agg.agreement,
            agg.prob
        ));
    }
    if calibrated.is_empty() {
        trust.push_str("Probabilities are uncalibrated: they are what the providers state, not a measured hit rate. ");
    } else {
        trust.push_str(&format!("Calibrated judges: {}. ", calibrated.join(", ")));
    }
    if panel.judges.len() < 3 {
        trust.push_str("A panel of fewer than three judges gives agreement little meaning. ");
    }
    json!({
        "status": if agg.escalated { "needs_human" } else { "decided" },
        "answer": answer,
        "prob": agg.prob,
        "agreement": agg.agreement,
        "answering_judges": agg.answering,
        "abstained_judges": agg.abstained,
        "reasons": agg.reasons,
        "kind": kind.as_str(),
        "trust": trust.trim_end(),
    })
}

fn ranking_value(plan: &Plan, outcomes: &[ItemOutcome]) -> Value {
    let pair_items: Vec<&ItemOutcome> = outcomes.iter().filter(|o| o.item.pair.is_some()).collect();
    if pair_items.is_empty() {
        return Value::Null;
    }
    let n = pair_items
        .iter()
        .filter_map(|o| o.item.pair)
        .map(|(i, j)| i.max(j) + 1)
        .max()
        .unwrap_or(0);
    let mut votes = Vec::new();
    let mut undecided = 0;
    for o in &pair_items {
        let (i, j) = o.item.pair.unwrap_or((0, 0));
        for v in &o.votes {
            match v.answer.as_deref() {
                Some("a") => votes.push((i, j, 1.0)),
                Some("b") => votes.push((i, j, 0.0)),
                Some("tie") => votes.push((i, j, 0.5)),
                _ => undecided += 1,
            }
        }
    }
    let bt = bradley_terry(n, &votes, 300, 0x00F1_1BD1);
    let _ = plan;
    json!({
        "method": "bradley_terry",
        "votes_used": votes.len(),
        "pairs_undecided": undecided,
        "candidates": (0..n).map(|i| json!({
            "index": i,
            "strength": (bt.strength[i] * 1e4).round() / 1e4,
            "ci95": [(bt.ci[i].0 * 1e4).round() / 1e4, (bt.ci[i].1 * 1e4).round() / 1e4],
            "rank_ci95": [bt.rank_ci[i].0, bt.rank_ci[i].1],
            "comparisons": bt.comparisons[i],
        })).collect::<Vec<_>>(),
        "warnings": bt.warnings,
    })
}

// ---- recording --------------------------------------------------------------------------

/// Records a run's answers as proposals through the `decide` path.
///
/// Individual answers and panel aggregates are advice only, including human
/// source labels. Escalations remain unresolved; no confidence gate is used.
pub fn record(
    run: &RunResult,
    report_json: &Path,
    report: &Report,
    _cfg: &DecisionsConfig,
) -> Vec<Value> {
    let mut rows = Vec::new();
    for o in &run.outcomes {
        let JudgeQuestion::Decision(q) = o.item.question else {
            continue;
        };
        let mut push = |source: &str, question: Question, answer: &str, prob: f64, note: String| {
            let a = Answer {
                entry: o.item.entry.clone(),
                question,
                hotspot: o.item.hotspot,
                answer: answer.to_owned(),
                prob: Some(prob.clamp(0.0, 1.0)),
                confidence: None,
                source: source.to_owned(),
                note,
                request_hash: Some(o.item.request_hash.clone()),
            };
            let result = crate::decision::propose_report(report_json, report, &a);
            rows.push(match result {
                Ok(out) => {
                    json!({"entry": a.entry, "source": source, "question": question.as_str(),
                    "answer": answer, "decided": out.decided, "reason": out.reason})
                }
                Err(e) => json!({"entry": a.entry, "source": source, "question": question.as_str(),
                    "answer": answer, "error": e.to_string()}),
            });
        };
        for v in &o.votes {
            if let Some(a) = v.answer.as_deref().filter(|a| !is_abstain(a)) {
                let prob = v.probs.get(a).copied().unwrap_or(1.0);
                let note = format!(
                    "run={};{}",
                    run.value["run_id"].as_str().unwrap_or(""),
                    o.notes.get(&v.judge).cloned().unwrap_or_default()
                );
                push(&v.judge, q, a, prob, note);
            }
        }
        let note = format!(
            "run={};agreement={:.2};judges={}",
            run.value["run_id"].as_str().unwrap_or(""),
            o.aggregate.agreement,
            o.aggregate.answering
        );
        match &o.aggregate.answer {
            Some(a) => push(PANEL_SOURCE, q, a, o.aggregate.prob, note),
            None => {
                let note = format!("escalated: {}", o.aggregate.reasons.join("; "));
                if q == Question::Accept {
                    push(PANEL_SOURCE, q, "needs_human", 1.0, note);
                } else {
                    push(PANEL_SOURCE, Question::AskHuman, "yes", 1.0, note);
                }
            }
        }
    }
    let _ = report;
    rows
}

// ---- self-test ------------------------------------------------------------------------------

/// Options of [`selftest`].
#[derive(Debug, Clone)]
pub struct SelftestOptions {
    /// Most items re-asked per judge.
    pub items: usize,
    /// Pixels the hotspot crops and colour regions are shifted by.
    pub offset_px: u32,
    /// Most calls the self-test may make.
    pub max_calls: usize,
}

#[cfg(feature = "evaluation")]
fn renamed(item: &JudgeItem) -> JudgeItem {
    let mut it = item.clone();
    let alias = format!("item_{}.png", &sha(&item.id)[7..13]);
    for s in &mut it.states {
        if let Some(o) = s.as_object_mut() {
            o.insert("entry".into(), json!(alias));
        }
    }
    it.entry = alias;
    it
}

#[cfg(feature = "evaluation")]
fn shifted(item: &JudgeItem, dx: u32) -> JudgeItem {
    let mut it = item.clone();
    for h in &mut it.hotspots {
        h.rect_px[0] = h.rect_px[0].saturating_add(dx);
        h.rect_px[1] = h.rect_px[1].saturating_add(dx);
    }
    if let Some((a, b)) = item.source.as_ref().and_then(PixelSource::load) {
        let (w, h) = a.dimensions();
        let map = vec![1.0f32; (w * h) as usize];
        let sh = colour_shifts(&a, &b, &map, &it.hotspots, 3);
        for s in &mut it.states {
            if let (Some(o), Ok(v)) = (s.as_object_mut(), serde_json::to_value(&sh)) {
                o.insert("colour_shifts".into(), v);
            }
        }
    }
    it
}

/// Re-asks each judge the same questions with irrelevant perturbations (a
/// repeat as the noise floor, the A/B order swapped, the entry renamed, the
/// crop shifted) and reports how often the answer changes.
#[cfg(feature = "evaluation")]
pub fn selftest(
    items: &[JudgeItem],
    panel: &Panel,
    backend: &dyn Backend,
    opts: &SelftestOptions,
    dry_run: bool,
) -> Value {
    let items: Vec<&JudgeItem> = items
        .iter()
        .filter(|i| i.canary.is_none())
        .take(opts.items)
        .collect();
    let mut rows = Vec::new();
    let mut planned = 0usize;
    let mut calls = 0usize;
    for spec in panel
        .judges
        .iter()
        .filter(|j| j.provider != Provider::Human)
    {
        let mut counts: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
        let mut abstained = 0usize;
        let mut asked = 0usize;
        for item in items.iter().filter(|i| spec.answers(i.question)) {
            let variants: [(&str, JudgeItem, Order); 4] = [
                ("repeat", (*item).clone(), Order::Ab),
                ("order_swap", (*item).clone(), Order::Ba),
                ("renamed_entry", renamed(item), Order::Ab),
                ("shifted_crop", shifted(item, opts.offset_px), Order::Ab),
            ];
            planned += 5;
            if dry_run {
                continue;
            }
            let base = judge_one(backend, spec, item, Order::Ab, &mut calls, opts.max_calls);
            asked += 1;
            let key = |j: &Judgement| j.answer.clone().filter(|a| !is_abstain(a));
            if key(&base).is_none() {
                abstained += 1;
                continue;
            }
            for (name, it, order) in &variants {
                let j = judge_one(backend, spec, it, *order, &mut calls, opts.max_calls);
                let e = counts.entry(name).or_insert((0, 0));
                if let Some(a) = key(&j) {
                    e.0 += 1;
                    e.1 += usize::from(Some(a) != key(&base));
                }
            }
        }
        let rate = |n: &str| -> Value {
            counts
                .get(n)
                .filter(|c| c.0 > 0)
                .map_or(Value::Null, |c| json!(c.1 as f64 / c.0 as f64))
        };
        let floor = counts
            .get("repeat")
            .filter(|c| c.0 > 0)
            .map(|c| c.1 as f64 / c.0 as f64);
        let worst = ["order_swap", "renamed_entry", "shifted_crop"]
            .iter()
            .filter_map(|n| {
                counts
                    .get(n)
                    .filter(|c| c.0 > 0)
                    .map(|c| c.1 as f64 / c.0 as f64)
            })
            .fold(0.0f64, f64::max);
        rows.push(json!({
            "judge": spec.id, "items_asked": asked, "base_abstained": abstained,
            "flip_rate": {
                "repeat_noise_floor": rate("repeat"), "order_swap": rate("order_swap"),
                "renamed_entry": rate("renamed_entry"), "shifted_crop": rate("shifted_crop"),
            },
            "stable": if dry_run { Value::Null } else { json!(worst <= floor.unwrap_or(0.0) + 0.1) },
        }));
    }
    json!({
        "schema": SELFTEST_SCHEMA,
        "dry_run": dry_run,
        "items": items.len(),
        "calls_planned": planned,
        "calls_made": calls,
        "perturbations": {
            "repeat": "the same request again: the judge's own nondeterminism, the floor the others are read against",
            "order_swap": "the reference is presented second instead of first",
            "renamed_entry": "the entry name is replaced by a neutral alias",
            "shifted_crop": format!("hotspot crops and colour regions shifted by {} px", opts.offset_px),
        },
        "judges": rows,
        "reading": "a flip rate above the repeat floor means the answer depends on something irrelevant to the images; trust such a judge less on every question",
    })
}

/// The map from a vote-page answer back to a stable answer is the same as
/// for a model; exposed for the vote store.
pub(crate) fn stable_for(item: &JudgeItem, order: &str, wire: &str) -> String {
    stable_answer(
        item,
        if order == "ba" { Order::Ba } else { Order::Ab },
        wire,
    )
}

/// Strips (PNG bytes) of an item for the vote page.
pub(crate) fn vote_strips(item: &JudgeItem, order: &str) -> Vec<Vec<u8>> {
    strips_for(item, if order == "ba" { Order::Ba } else { Order::Ab })
}
