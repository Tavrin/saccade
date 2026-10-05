//! Bounded decisions: fixed questions about a report entry that an agent, a
//! bounded-decision model (Jev, the OpenAI Decisions API) or a person answers
//! from a small closed set, and the file those answers are recorded in.
//!
//! [`build_request`] turns a [`Report`] into a `saccade-decision-request.v1`
//! document: per entry, a stable question, its allowed answers and a compact
//! `state`. The output is deterministic, so the same report gives the same
//! bytes and each item's `request_hash` identifies exactly what was asked.
//!
//! [`decide_report`] records attributed proposals only. Source labels, confidence,
//! calibration and panel agreement cannot create a final decision. Baseline
//! updates consume a separate, exact-content-bound CLI or workbench disposition.

use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

use crate::error::Error;
use crate::report::{Entry, Mode, Report, Status};
use crate::view::{DECISIONS_SCHEMA, Decisions, Proposal, SetDecision, Verdict, read_decisions};

/// Schema identifier of a decision request.
pub const REQUEST_SCHEMA: &str = "saccade-decision-request.v1";

/// File name of the decisions file inside a report directory.
pub const DECISIONS_FILE_NAME: &str = "saccade-decisions.v1.json";

/// File name of the script twin of the decisions file. The HTML pages load it
/// with a `<script>` tag, which works from `file://` where `fetch` does not.
pub const DECISIONS_SIDECAR_NAME: &str = "saccade-decisions.v1.js";

/// Most hotspots listed in an item's state.
const STATE_HOTSPOTS: usize = 3;

/// Most `mask_suggest` items one entry produces.
const MASK_HOTSPOTS: usize = 5;

/// Empty legacy configuration table. Promotion settings are no longer accepted.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecisionsConfig {}

/// A decision-request question.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Question {
    /// Is the change intended (`accept`), a regression (`reject`) or unclear?
    Accept,
    /// What kind of difference is it?
    Triage,
    /// What most likely caused it?
    Cause,
    /// Does a person have to look at it?
    AskHuman,
    /// Is a hotspot noise worth masking, or a real change?
    MaskSuggest,
}

impl Question {
    /// Every question type, in documentation order.
    pub const ALL: [Self; 5] = [
        Self::Accept,
        Self::Triage,
        Self::Cause,
        Self::AskHuman,
        Self::MaskSuggest,
    ];

    /// The name used on the command line and in JSON.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Accept => "accept",
            Self::Triage => "triage",
            Self::Cause => "cause",
            Self::AskHuman => "ask_human",
            Self::MaskSuggest => "mask_suggest",
        }
    }

    /// Parses a question name.
    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|q| q.as_str() == s)
    }

    /// The fixed wording of the question. Never varies with the entry.
    pub fn text(self) -> &'static str {
        match self {
            Self::Accept => {
                "Is this visual change intended, or a regression? Judge it against the intent text when present."
            }
            Self::Triage => "What kind of difference separates the capture from the baseline?",
            Self::Cause => "What is the most likely cause of this difference?",
            Self::AskHuman => {
                "Is this case ambiguous enough that a person must look at it before anything is decided?"
            }
            Self::MaskSuggest => {
                "Is this hotspot noise that a mask or region should exclude, or a real change?"
            }
        }
    }

    /// The closed set of answers.
    pub fn allowed_answers(self) -> &'static [&'static str] {
        match self {
            Self::Accept => &["accept", "reject", "needs_human"],
            Self::Triage => &[
                "noise",
                "local_defect",
                "global_shift",
                "config_mismatch",
                "broken_frame",
            ],
            Self::Cause => &[
                "global_tone",
                "local_structure",
                "misaligned",
                "noise",
                "broken_frame",
                "config_mismatch",
            ],
            Self::AskHuman => &["yes", "no"],
            Self::MaskSuggest => &["noise_region", "real_change"],
        }
    }
}

/// Why a decision could not be requested or recorded.
#[derive(Debug)]
pub enum DecideError {
    /// A model may not answer this entry (a deterministic failure).
    Refused(String),
    /// The request itself is wrong: unknown entry, answer outside the allowed set.
    Invalid(String),
    /// A file or JSON problem.
    Other(Error),
}

impl std::fmt::Display for DecideError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Refused(m) | Self::Invalid(m) => f.write_str(m),
            Self::Other(e) => e.fmt(f),
        }
    }
}

impl From<Error> for DecideError {
    fn from(e: Error) -> Self {
        Self::Other(e)
    }
}

impl From<serde_json::Error> for DecideError {
    fn from(e: serde_json::Error) -> Self {
        Self::Other(Error::Json(e))
    }
}

/// Rounds to 6 decimals so the output does not depend on float noise.
fn r6(v: f64) -> Value {
    if v.is_finite() {
        json!((v * 1e6).round() / 1e6)
    } else {
        Value::Null
    }
}

/// Why a model may not answer `e`, or `None` when it may. These are facts the
/// run established; no probability overrides them.
pub fn deterministic_failure(report: &Report, e: &Entry) -> Option<String> {
    if e.status == Status::Error {
        let why = e
            .error
            .as_deref()
            .unwrap_or("the pair could not be compared");
        return Some(if !e.meta_diff.is_empty() && report.config.meta.required {
            format!("config mismatch under --require-matching-meta ({why})")
        } else {
            format!("error status ({why})")
        });
    }
    for (side, p) in [
        ("capture", &e.properties),
        ("baseline", &e.baseline_properties),
    ] {
        if p.is_some_and(|p| p.nan_count + p.inf_count > 0) {
            return Some(format!(
                "the {side} has non-finite (NaN or infinite) samples"
            ));
        }
    }
    let base = e.baseline_properties.as_ref();
    if let Some(p) = &e.properties {
        if p.is_all_black && !base.is_some_and(|b| b.is_all_black) {
            return Some("broken frame: the capture is all black".to_owned());
        }
        if p.is_all_white && !base.is_some_and(|b| b.is_all_white) {
            return Some("broken frame: the capture is all white".to_owned());
        }
    }
    if report.config.mode == Mode::Identity && e.status == Status::Fail {
        return Some("identity break: the candidate differs from its parent".to_owned());
    }
    None
}

fn status_name(s: Status) -> &'static str {
    match s {
        Status::Pass => "pass",
        Status::Fail => "fail",
        Status::New => "new",
        Status::Missing => "missing",
        Status::Error => "error",
    }
}

fn metric_name(m: crate::report::Metric) -> &'static str {
    match m {
        crate::report::Metric::Mean => "mean",
        crate::report::Metric::P95 => "p95",
        crate::report::Metric::P99 => "p99",
        crate::report::Metric::Max => "max",
    }
}

fn props_value(p: &crate::report::Properties) -> Value {
    json!({
        "all_black": p.is_all_black,
        "all_white": p.is_all_white,
        "nan": p.nan_count,
        "inf": p.inf_count,
    })
}

/// The compact, model-friendly state of one entry: numbers and flags only, no
/// paths and no free text beyond the intent.
pub(crate) fn entry_state(report: &Report, e: &Entry, intent: Option<&str>) -> Value {
    let mut s = Map::new();
    s.insert("entry".into(), json!(e.name));
    s.insert("verdict".into(), json!(status_name(e.status)));
    s.insert(
        "mode".into(),
        json!(if report.config.mode == Mode::Identity {
            "identity"
        } else {
            "regression"
        }),
    );
    s.insert("metric".into(), json!(metric_name(e.metric_used)));
    s.insert("threshold".into(), r6(e.threshold));
    if let Some(v) = e.value {
        s.insert("value".into(), r6(v));
    }
    if let Some(m) = &e.metrics {
        s.insert(
            "metrics".into(),
            json!({
                "mean": r6(m.mean), "p95": r6(m.p95), "p99": r6(m.p99), "max": r6(m.max),
                "frac_above_0_1": r6(m.frac_above_0_1), "frac_above_0_5": r6(m.frac_above_0_5),
                "width": m.width, "height": m.height,
            }),
        );
    }
    let hs: Vec<Value> = e
        .hotspots
        .iter()
        .take(STATE_HOTSPOTS)
        .enumerate()
        .map(|(i, h)| {
            json!({
                "n": i + 1, "position": h.position, "size": [h.rect_px[2], h.rect_px[3]],
                "share": r6(h.share_of_total_error), "max": r6(h.max_flip), "mean": r6(h.mean_flip),
            })
        })
        .collect();
    s.insert("hotspots".into(), Value::Array(hs));
    let frame_wide = e
        .hotspots
        .first()
        .is_some_and(|h| h.rect_frac[2] * h.rect_frac[3] >= 0.5);
    s.insert("frame_wide".into(), json!(frame_wide));
    let mut props = Map::new();
    if let Some(p) = &e.properties {
        props.insert("capture".into(), props_value(p));
    }
    if let Some(p) = &e.baseline_properties {
        props.insert("baseline".into(), props_value(p));
    }
    s.insert("properties".into(), Value::Object(props));
    let declared: Vec<globset::GlobMatcher> = report
        .config
        .meta
        .declared
        .iter()
        .filter_map(|g| crate::config::compile_glob(g).ok())
        .collect();
    let meta: Vec<Value> = e
        .meta_diff
        .iter()
        .map(|d| json!({"key": d.key, "declared": declared.iter().any(|g| g.is_match(&d.key))}))
        .collect();
    s.insert("meta_diff".into(), Value::Array(meta));
    if let Some(b) = e.bit_identical {
        s.insert("bit_identical".into(), json!(b));
    }
    if let Some(d) = &e.diagnostics {
        let mut dv = Map::new();
        dv.insert("class".into(), json!(d.class.as_str()));
        dv.insert("description".into(), json!(d.description));
        if let Some(t) = &d.tone {
            dv.insert(
                "tone_explained_fraction".into(),
                r6(t.tone_explained_fraction),
            );
        }
        if let Some(f) = d.shift.as_ref().and_then(|x| x.shift_explained_fraction) {
            dv.insert("shift_explained_fraction".into(), r6(f));
        }
        s.insert("diagnostics".into(), Value::Object(dv));
    }
    if let Some(i) = intent {
        s.insert("intent".into(), json!(i));
    }
    Value::Object(s)
}

/// The `request_hash` of an item: SHA-256 over the canonical JSON of what the
/// answerer saw (question type, wording, allowed answers, state).
fn item_hash(question: Question, hotspot: Option<usize>, state: &Value) -> String {
    let canonical = json!({
        "question_type": question.as_str(),
        "question": question.text(),
        "allowed_answers": question.allowed_answers(),
        "hotspot": hotspot,
        "state": state,
    });
    let digest = Sha256::digest(canonical.to_string().as_bytes());
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    format!("sha256:{hex}")
}

fn make_item(question: Question, name: &str, hotspot: Option<usize>, mut state: Value) -> Value {
    if let (Some(n), Some(obj)) = (hotspot, state.as_object_mut()) {
        obj.insert("hotspot".into(), json!(n));
    }
    let hash = item_hash(question, hotspot, &state);
    let mut item = Map::new();
    item.insert(
        "id".into(),
        json!(match hotspot {
            Some(n) => format!("{name}#{n}"),
            None => name.to_owned(),
        }),
    );
    item.insert("entry".into(), json!(name));
    if let Some(n) = hotspot {
        item.insert("hotspot".into(), json!(n));
    }
    item.insert("question_type".into(), json!(question.as_str()));
    item.insert("question".into(), json!(question.text()));
    item.insert("allowed_answers".into(), json!(question.allowed_answers()));
    item.insert("state".into(), state);
    item.insert("request_hash".into(), json!(hash));
    Value::Object(item)
}

/// Which entries a request covers.
#[derive(Debug, Clone, Default)]
pub struct RequestOptions {
    /// The question type to ask.
    pub question: Option<Question>,
    /// Explicit entry names; empty with `all_failing` false is an error.
    pub entries: Vec<String>,
    /// Every entry with status `fail` that a model may answer.
    pub all_failing: bool,
    /// What the change is for (a commit message or PR text).
    pub intent: Option<String>,
}

/// Builds the `saccade-decision-request.v1` document for `report`.
///
/// Entries a model may not answer ([`deterministic_failure`]) are listed under
/// `skipped` with the reason instead of becoming items; naming such an entry
/// explicitly is an error.
pub fn build_request(report: &Report, opts: &RequestOptions) -> Result<Value, DecideError> {
    let question = opts.question.unwrap_or(Question::Accept);
    let mut chosen: Vec<&Entry> = Vec::new();
    for name in &opts.entries {
        let e = report
            .entries
            .iter()
            .find(|e| &e.name == name)
            .ok_or_else(|| DecideError::Invalid(format!("the report has no entry {name:?}")))?;
        if let Some(why) = deterministic_failure(report, e) {
            return Err(DecideError::Refused(format!(
                "{name}: {why}; this is a deterministic failure, not a question for a model"
            )));
        }
        chosen.push(e);
    }
    let mut skipped = Vec::new();
    if opts.all_failing {
        for e in report.entries.iter().filter(|e| e.status == Status::Fail) {
            match deterministic_failure(report, e) {
                Some(why) => skipped.push(json!({"entry": e.name, "reason": why})),
                None if !chosen.iter().any(|c| c.name == e.name) => chosen.push(e),
                None => {}
            }
        }
    }
    if opts.entries.is_empty() && !opts.all_failing {
        return Err(DecideError::Invalid(
            "name an entry (--entry) or ask for every failing one (--all-failing)".into(),
        ));
    }
    let intent = opts.intent.as_deref().filter(|i| !i.trim().is_empty());
    let mut items = Vec::new();
    for e in chosen {
        let state = entry_state(report, e, intent);
        if question == Question::MaskSuggest {
            for n in 1..=e.hotspots.len().min(MASK_HOTSPOTS) {
                items.push(make_item(question, &e.name, Some(n), state.clone()));
            }
        } else {
            items.push(make_item(question, &e.name, None, state));
        }
    }
    Ok(json!({
        "schema": REQUEST_SCHEMA,
        "question_type": question.as_str(),
        "items": items,
        "skipped": skipped,
    }))
}

/// One answer to record.
#[derive(Debug, Clone)]
pub struct Answer {
    /// The entry (image set) it is about.
    pub entry: String,
    /// The question it answers.
    pub question: Question,
    /// For `mask_suggest`: the 1-based hotspot.
    pub hotspot: Option<u32>,
    /// One of the question's allowed answers.
    pub answer: String,
    /// The answerer's confidence in `[0, 1]`; required for a model.
    pub prob: Option<f64>,
    /// An additional confidence figure, stored beside `prob`.
    pub confidence: Option<f64>,
    /// `jev`, `openai-decisions`, a model name or `human`.
    pub source: String,
    /// Free-text note.
    pub note: String,
    /// The request item's hash; computed from the report (without an intent)
    /// when absent.
    pub request_hash: Option<String>,
}

/// What recording an answer did.
#[derive(Debug, Clone)]
pub struct Outcome {
    /// The decisions file written.
    pub file: PathBuf,
    /// Always true: source labels cannot confer approval authority.
    pub proposed: bool,
    /// Always false: recording an answer never creates a final decision.
    pub decided: bool,
    /// The set's decision after recording.
    pub decision: Option<Verdict>,
    /// Why a proposal stayed a proposal, when it did.
    pub reason: Option<String>,
}

fn is_human(source: &str) -> bool {
    crate::judge_stats::is_human_source(source)
}

fn validate(a: &Answer) -> Result<(), DecideError> {
    if !a.question.allowed_answers().contains(&a.answer.as_str()) {
        return Err(DecideError::Invalid(format!(
            "{:?} is not an allowed answer to the {} question; allowed: {}",
            a.answer,
            a.question.as_str(),
            a.question.allowed_answers().join(", ")
        )));
    }
    if a.question == Question::MaskSuggest && a.hotspot.is_none_or(|n| n == 0) {
        return Err(DecideError::Invalid(
            "a mask_suggest answer needs the 1-based hotspot it is about".into(),
        ));
    }
    if a.source.trim().is_empty() {
        return Err(DecideError::Invalid("the answer needs a source".into()));
    }
    match a.prob {
        Some(p) if !(0.0..=1.0).contains(&p) || !p.is_finite() => {
            return Err(DecideError::Invalid(format!("prob {p} is not in [0, 1]")));
        }
        None if !is_human(&a.source) => {
            return Err(DecideError::Invalid(
                "a model's answer needs its probability (prob, 0 to 1)".into(),
            ));
        }
        _ => {}
    }
    Ok(())
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

/// Applies `a` to `d`, returning the outcome (without a file path).
fn apply(d: &mut Decisions, a: &Answer, hash: String) -> (bool, Option<Verdict>, Option<String>) {
    let ms = now_ms();
    let idx = match d.sets.iter().position(|s| s.name == a.entry) {
        Some(i) => i,
        None => {
            d.sets.push(SetDecision {
                name: a.entry.clone(),
                decision: None,
                chosen_label: None,
                no_difference: false,
                note: String::new(),
                roi: None,
                timestamp_ms: ms,
                chosen_dir: None,
                sha256: Vec::new(),
                proposals: Vec::new(),
            });
            d.sets.len() - 1
        }
    };
    let set = &mut d.sets[idx];
    set.timestamp_ms = ms;
    let p = Proposal {
        question: a.question.as_str().to_owned(),
        hotspot: a.hotspot,
        answer: a.answer.clone(),
        prob: a.prob,
        confidence: a.confidence,
        source: a.source.trim().to_owned(),
        timestamp_ms: ms,
        request_hash: hash,
        note: a.note.clone(),
        proposed: true,
        promoted: false,
    };
    set.proposals
        .retain(|q| !(q.question == p.question && q.hotspot == p.hotspot && q.source == p.source));
    set.proposals.push(p);
    (
        false,
        set.decision,
        Some(
            "answers are proposals; an explicit hash-bound disposition is required for approval"
                .into(),
        ),
    )
}

/// Writes the decisions file and its script twin atomically.
fn write_decisions(path: &Path, d: &Decisions, sidecar: bool) -> Result<(), DecideError> {
    let io = |what: String| {
        move |source| Error::Io {
            context: what,
            source,
        }
    };
    let text = serde_json::to_string_pretty(d)?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, format!("{text}\n")).map_err(io(format!("writing {}", tmp.display())))?;
    std::fs::rename(&tmp, path).map_err(io(format!("replacing {}", path.display())))?;
    if sidecar {
        let js = path.with_file_name(DECISIONS_SIDECAR_NAME);
        // `<\/` keeps the text from closing a script element it is embedded in.
        let body = serde_json::to_string(d)?.replace("</", "<\\/");
        std::fs::write(&js, format!("window.__saccadeDecisions={body};\n"))
            .map_err(io(format!("writing {}", js.display())))?;
    }
    Ok(())
}

/// Adds to `incoming` the proposals of `on_disk` it does not carry (matched by
/// set, question, hotspot, source and time), so a page saving its own state
/// never drops an answer `saccade decide` recorded in the meantime.
pub fn merge_proposals(incoming: &mut Decisions, on_disk: &Decisions) {
    for old in &on_disk.sets {
        for p in &old.proposals {
            let set = match incoming.sets.iter_mut().find(|s| s.name == old.name) {
                Some(s) => s,
                None => {
                    let mut empty = old.clone();
                    empty.proposals.clear();
                    incoming.sets.push(empty);
                    let last = incoming.sets.len() - 1;
                    &mut incoming.sets[last]
                }
            };
            let known = set.proposals.iter().any(|q| {
                q.question == p.question
                    && q.hotspot == p.hotspot
                    && q.source == p.source
                    && q.timestamp_ms >= p.timestamp_ms
            });
            if !known {
                set.proposals.retain(|q| {
                    !(q.question == p.question && q.hotspot == p.hotspot && q.source == p.source)
                });
                set.proposals.push(p.clone());
            }
        }
    }
}

/// Writes the empty script twin of the decisions file when there is none, so
/// the page's `<script>` load never fails. Errors are ignored: the page works
/// without it.
pub(crate) fn ensure_sidecar(dir: &Path) {
    let js = dir.join(DECISIONS_SIDECAR_NAME);
    if !js.exists() {
        let _ = std::fs::write(js, "window.__saccadeDecisions=null;\n");
    }
}

/// A decisions file for the sides of `report`, empty of sets.
fn skeleton(report: &Report) -> Decisions {
    let dirs = match (&report.baseline_dir, &report.capture_dir) {
        (Some(b), Some(c)) => vec![b.clone(), c.clone()],
        _ => Vec::new(),
    };
    Decisions {
        reviewer_exposure: None,
        saw_model_proposals: None,
        schema: DECISIONS_SCHEMA.to_owned(),
        seed: 0,
        labels: vec![
            report.config.labels.baseline.clone(),
            report.config.labels.capture.clone(),
        ],
        blind: false,
        dirs,
        sets: Vec::new(),
    }
}

/// Records `answer` for a report. The decisions file is
/// `<report_dir>/saccade-decisions.v1.json`, created when missing.
pub fn decide_report(
    report_json: &Path,
    report: &Report,
    cfg: &DecisionsConfig,
    answer: &Answer,
) -> Result<Outcome, DecideError> {
    let _ = cfg;
    decide_report_inner(report_json, report, answer)
}

/// Records a panel member or aggregate as a proposal, including human source labels.
pub fn propose_report(
    report_json: &Path,
    report: &Report,
    answer: &Answer,
) -> Result<Outcome, DecideError> {
    decide_report_inner(report_json, report, answer)
}

fn decide_report_inner(
    report_json: &Path,
    report: &Report,
    answer: &Answer,
) -> Result<Outcome, DecideError> {
    validate(answer)?;
    let entry = report
        .entries
        .iter()
        .find(|e| e.name == answer.entry)
        .ok_or_else(|| {
            DecideError::Invalid(format!("the report has no entry {:?}", answer.entry))
        })?;
    if let Some(why) = deterministic_failure(report, entry) {
        return Err(DecideError::Refused(format!(
            "{}: {why}; a model cannot decide this, a person must",
            answer.entry
        )));
    }
    let dir = report_json
        .parent()
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
    let file = dir.join(DECISIONS_FILE_NAME);
    let mut d = if file.is_file() {
        read_decisions(&file)?
    } else {
        skeleton(report)
    };
    let hash = match &answer.request_hash {
        Some(h) => h.clone(),
        None => {
            let state = entry_state(report, entry, None);
            let state = match (answer.hotspot, state) {
                (Some(n), Value::Object(mut o)) => {
                    o.insert("hotspot".into(), json!(n));
                    Value::Object(o)
                }
                (_, s) => s,
            };
            item_hash(answer.question, answer.hotspot.map(|n| n as usize), &state)
        }
    };
    let (decided, decision, reason) = apply(&mut d, answer, hash);
    // The images the answer is about, so `approve` can check what it copies.
    if let Some(set) = d.sets.iter_mut().find(|s| s.name == answer.entry)
        && set.sha256.is_empty()
        && d.dirs.len() == 2
    {
        set.sha256 = vec![entry.baseline_sha256.clone(), entry.capture_sha256.clone()];
    }
    write_decisions(&file, &d, true)?;
    Ok(Outcome {
        file,
        proposed: true,
        decided,
        decision,
        reason,
    })
}

/// A decisions file for a view directory, empty of sets, from the model
/// embedded in its page.
pub fn view_skeleton(view_dir: &Path) -> Result<Decisions, Error> {
    let m = crate::snapshot::view_model(view_dir)?;
    let strings = |k: &str| -> Vec<String> {
        m[k].as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default()
    };
    Ok(Decisions {
        reviewer_exposure: None,
        saw_model_proposals: None,
        schema: DECISIONS_SCHEMA.to_owned(),
        seed: m["seed"].as_u64().unwrap_or(0),
        labels: strings("labels"),
        blind: m["blind"].as_bool().unwrap_or(false),
        dirs: strings("dirs"),
        sets: Vec::new(),
    })
}

/// Records `answer` into a decisions file: a serve session's, one exported
/// from a viewer, or (with `skeleton`, when the file does not exist yet) the
/// one of a view directory. Every answer stays a proposal.
pub fn decide_file(
    file: &Path,
    answer: &Answer,
    skeleton: Option<Decisions>,
) -> Result<Outcome, DecideError> {
    validate(answer)?;
    let mut d = match (file.is_file(), skeleton) {
        (false, Some(s)) => s,
        _ => read_decisions(file)?,
    };
    let hash = answer.request_hash.clone().unwrap_or_default();
    let (decided, decision, reason) = apply(&mut d, answer, hash);
    let sidecar = file.file_name().is_some_and(|n| n == DECISIONS_FILE_NAME);
    write_decisions(file, &d, sidecar)?;
    Ok(Outcome {
        file: file.to_path_buf(),
        proposed: true,
        decided,
        decision,
        reason,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::report::*;

    fn entry(name: &str, status: Status) -> Entry {
        Entry {
            intended_variables: Vec::new(),
            spatial: None,
            gallery: Vec::new(),
            layers: None,
            required_effects: Vec::new(),
            buffer: None,
            name: name.into(),
            status,
            metric_used: Metric::Mean,
            threshold: 0.01,
            value: Some(0.05),
            metrics: None,
            properties: None,
            paths: EntryPaths::default(),
            error: None,
            regions: Vec::new(),
            masked_fraction: None,
            pixel_exclusions: None,
            sample_exclusions: None,
            bit_identical: Some(false),
            hdr: None,
            meta_diff: Vec::new(),
            meta_ignored_diff: Vec::new(),
            file_bytes_identical: None,
            capture_validity: Default::default(),
            capture_provenance: Default::default(),
            meta_declared_unchanged: Vec::new(),
            baseline_properties: None,
            warnings: Vec::new(),
            baseline_sha256: Some("aa".into()),
            capture_sha256: Some("bb".into()),
            hotspots: Vec::new(),
            changed_pixel_runs: Vec::new(),
            object_attribution: Vec::new(),
            pass_with_local_change: false,
            diagnostics: None,
        }
    }

    fn report(entries: Vec<Entry>, mode: Mode) -> Report {
        let mut t = Totals::default();
        for e in &entries {
            t.total += 1;
            match e.status {
                Status::Pass => t.pass += 1,
                Status::Fail => t.fail += 1,
                Status::New => t.new += 1,
                Status::Missing => t.missing += 1,
                Status::Error => t.error += 1,
            }
        }
        Report {
            perf_diff: None,
            perf_errors: Vec::new(),
            combined_verdict: None,
            exclusion_audit: None,
            schema: REPORT_SCHEMA.into(),
            tool_version: "0".into(),
            generated_at_unix: 0,
            baseline_dir: Some("/b".into()),
            capture_dir: Some("/c".into()),
            config: ReportConfig {
                mask_mode: crate::compare::MaskMode::default(),
                entries: Vec::new(),
                ignore: Vec::new(),
                default_threshold: 0.01,
                default_metric: Metric::Mean,
                pixels_per_degree: 67.0,
                fail_on_new: false,
                mode,
                labels: Labels::default(),
                meta: MetaSettings::default(),
                allow_empty: false,
                fail_on_nonfinite: true,
                hotspot_fail: None,
                hotspot_local_max: 0.5,
                hotspot_local_min_pixels: 16,
            },
            totals: t,
            entries,
        }
    }

    fn answer(entry: &str, ans: &str, prob: f64, source: &str) -> Answer {
        Answer {
            entry: entry.into(),
            question: Question::Accept,
            hotspot: None,
            answer: ans.into(),
            prob: Some(prob),
            confidence: None,
            source: source.into(),
            note: String::new(),
            request_hash: None,
        }
    }

    #[test]
    fn request_is_byte_stable_and_hashed() {
        let r = report(vec![entry("a.png", Status::Fail)], Mode::Regression);
        let o = RequestOptions {
            all_failing: true,
            intent: Some("brighten the sky".into()),
            ..RequestOptions::default()
        };
        let one = build_request(&r, &o).unwrap().to_string();
        let two = build_request(&r, &o).unwrap().to_string();
        assert_eq!(one, two);
        let v: Value = serde_json::from_str(&one).unwrap();
        assert_eq!(v["schema"], REQUEST_SCHEMA);
        let item = &v["items"][0];
        assert_eq!(item["question"], Question::Accept.text());
        assert_eq!(
            item["allowed_answers"],
            json!(["accept", "reject", "needs_human"])
        );
        assert_eq!(item["state"]["intent"], "brighten the sky");
        assert!(
            item["request_hash"]
                .as_str()
                .unwrap()
                .starts_with("sha256:")
        );
    }

    #[test]
    fn sources_and_confidence_never_promote_answers() {
        let r = report(vec![entry("a.png", Status::Fail)], Mode::Regression);
        let tmp = tempfile::tempdir().unwrap();
        let rj = tmp.path().join(REPORT_FILE_NAME);
        for source in ["jev", "panel", "human", "human:forged"] {
            let out = decide_report(
                &rj,
                &r,
                &DecisionsConfig::default(),
                &answer("a.png", "accept", 1.0, source),
            )
            .unwrap();
            assert!(out.proposed && !out.decided && out.decision.is_none());
        }
        let saved = read_decisions(&tmp.path().join(DECISIONS_FILE_NAME)).unwrap();
        assert!(saved.accepted().is_empty());
        assert!(
            saved.sets[0]
                .proposals
                .iter()
                .all(|p| p.proposed && !p.promoted)
        );
        for settings in [
            "auto_accept_min_prob = 0.0",
            "allow_sources = ['human']",
            "gate_on = 'confidence'",
            "calibration = 'calibration.json'",
        ] {
            assert!(toml::from_str::<DecisionsConfig>(settings).is_err());
        }
        assert!(tmp.path().join(DECISIONS_SIDECAR_NAME).is_file());
    }

    #[test]
    fn deterministic_failures_are_refused_even_with_human_source_labels() {
        let mut broken = entry("b.png", Status::Fail);
        broken.properties = Some(Properties {
            is_all_black: true,
            is_all_white: false,
            mean_luminance: 0.0,
            min_luminance: 0.0,
            max_luminance: 0.0,
            nan_count: 0,
            inf_count: 0,
            negative_count: 0,
        });
        let r = report(vec![broken, entry("c.png", Status::Fail)], Mode::Identity);
        let tmp = tempfile::tempdir().unwrap();
        let rj = tmp.path().join(REPORT_FILE_NAME);
        let cfg = DecisionsConfig::default();
        for name in ["b.png", "c.png"] {
            let err =
                decide_report(&rj, &r, &cfg, &answer(name, "accept", 1.0, "jev")).unwrap_err();
            assert!(matches!(err, DecideError::Refused(_)), "{name}: {err}");
        }
        assert!(decide_report(&rj, &r, &cfg, &answer("b.png", "reject", 1.0, "human")).is_err());
    }
}
