//! Agent-facing commands: `snapshot` (a view state as a PNG), `decision-request`
//! (bounded questions about a report) and `decide` (record the answers), shared
//! by the CLI and the MCP tools `saccade_snapshot`, `saccade_decision_request`
//! and `saccade_decide`.

use std::io::Read;
use std::path::{Path, PathBuf};

use clap::Args;
use saccade_core::config::RunConfig;
use saccade_core::decision::{
    Answer, DecideError, DecisionsConfig, Outcome, Question, RequestOptions, build_request,
    decide_file, decide_report,
};
use saccade_core::report::{REPORT_FILE_NAME, Report};
use saccade_core::snapshot::{DEFAULT_WIDTH, ViewState, snapshot as draw, write_frames};
use saccade_core::view::VIEW_MARKER_FILE;
use serde_json::{Map, Value, json};

use crate::agent::CliError;

/// Schema identifier of what `decide` prints.
pub const DECIDE_RESULT_SCHEMA: &str = "saccade-decide-result.v1";

/// Widest image an MCP result carries.
#[cfg(feature = "mcp")]
const MCP_MAX_WIDTH: u32 = 1600;

impl From<DecideError> for CliError {
    fn from(e: DecideError) -> Self {
        match e {
            DecideError::Refused(m) => Self::new("deterministic_failure", m),
            DecideError::Invalid(m) => Self::usage(m),
            DecideError::Other(e) => e.into(),
        }
    }
}

/// `saccade snapshot`: render a view state to PNG, no browser.
#[derive(Args)]
pub struct SnapshotArgs {
    /// A `saccade-report.v1.json` or a view directory.
    pub target: PathBuf,
    /// The entry (report) or image set (view) to draw; default: the state's
    /// `set=`/`entry=`, else the first failing entry or the first set.
    #[arg(long)]
    pub entry: Option<String>,
    /// View state in the pages' hash grammar, for example
    /// `layout=swipe&split=0.3&zoom=4&at=120,80&heat=0.6&hotspot=1`.
    #[arg(long, value_name = "HASH")]
    pub state: Option<String>,
    /// PNG to write. The flicker layout writes `<stem>-1.png`, `<stem>-2.png`, ...
    #[arg(long, default_value = "snapshot.png")]
    pub out: PathBuf,
    /// Width of the stage in pixels.
    #[arg(long, default_value_t = DEFAULT_WIDTH)]
    pub width: u32,
    /// Print the written paths as JSON.
    #[arg(long)]
    pub json: bool,
}

/// `saccade decision-request`: bounded questions about a report's entries.
#[derive(Args)]
pub struct DecisionRequestArgs {
    /// Path to `saccade-report.v1.json`.
    pub report_json: PathBuf,
    /// Ask about this entry (repeatable).
    #[arg(long)]
    pub entry: Vec<String>,
    /// Ask about every failing entry a model may answer.
    #[arg(long)]
    pub all_failing: bool,
    /// Question type: accept, triage, cause, ask_human or mask_suggest.
    #[arg(long, default_value = "accept")]
    pub question: String,
    /// What the change is for (a commit message or PR text), shown to the answerer.
    #[arg(long)]
    pub intent: Option<String>,
}

/// `saccade decide`: record an answer to a decision-request question.
#[derive(Args)]
pub struct DecideArgs {
    /// A `saccade-report.v1.json`, a view directory, a decisions file (a
    /// serve session's) or a serve session id.
    pub target: PathBuf,
    /// The entry the answer is about.
    #[arg(long)]
    pub entry: Option<String>,
    /// The question answered: accept, triage, cause, ask_human or mask_suggest.
    #[arg(long, default_value = "accept")]
    pub question: String,
    /// For mask_suggest: the 1-based hotspot.
    #[arg(long)]
    pub hotspot: Option<u32>,
    /// The answer. Without it, answers are read from stdin as JSON, one
    /// object per line: `{"entry","answer","prob","source",...}`.
    #[arg(long)]
    pub answer: Option<String>,
    /// The answerer's probability, 0 to 1 (required for a model).
    #[arg(long)]
    pub prob: Option<f64>,
    /// Who answered: `jev`, `openai-decisions`, a model name or `human`.
    #[arg(long)]
    pub source: Option<String>,
    /// A second confidence figure the provider reports, stored beside `--prob`.
    #[arg(long)]
    pub confidence: Option<f64>,
    /// Free-text note.
    #[arg(long)]
    pub note: Option<String>,
    /// The `request_hash` of the item answered.
    #[arg(long)]
    pub request_hash: Option<String>,
    /// Config file with the `[decisions]` gate (default: `./saccade.toml` when present).
    #[arg(long)]
    pub config: Option<PathBuf>,
    /// Where `saccade serve` keeps decisions, for a session id target.
    #[arg(long)]
    pub decisions_dir: Option<PathBuf>,
    /// Print `saccade-decide-result.v1` objects, one per line.
    #[arg(long)]
    pub json: bool,
}

fn read_report(path: &Path) -> Result<Report, CliError> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| CliError::io(format!("reading report {}: {e}", path.display())))?;
    serde_json::from_str(&text)
        .map_err(|e| CliError::io(format!("parsing report {}: {e}", path.display())))
}

fn question_arg(name: &str) -> Result<Question, CliError> {
    Question::parse(name).ok_or_else(|| {
        CliError::usage(format!(
            "unknown question {name:?}; one of: {}",
            Question::ALL.map(Question::as_str).join(", ")
        ))
    })
}

fn decisions_config(path: Option<&Path>) -> Result<DecisionsConfig, CliError> {
    let path = match path {
        Some(p) => Some(p),
        None => Some(Path::new("saccade.toml")).filter(|p| p.is_file()),
    };
    Ok(match path {
        Some(p) => RunConfig::from_toml_file(p)?.decisions,
        None => DecisionsConfig::default(),
    })
}

fn pretty(value: &Value) -> Result<String, CliError> {
    Ok(format!("{}\n", serde_json::to_string_pretty(value)?))
}

fn emit(text: &str) -> Result<(), CliError> {
    use std::io::Write;
    let mut out = std::io::stdout().lock();
    match out.write_all(text.as_bytes()).and_then(|()| out.flush()) {
        Err(e) if e.kind() != std::io::ErrorKind::BrokenPipe => {
            Err(CliError::io(format!("writing to stdout: {e}")))
        }
        _ => Ok(()),
    }
}

// ---- snapshot -------------------------------------------------------------

/// What a snapshot wrote.
pub struct Snap {
    /// The PNG files, one per frame.
    pub paths: Vec<PathBuf>,
    /// Frame width in pixels.
    pub width: u32,
    /// Frame height in pixels.
    pub height: u32,
}

/// Draws `target` in state `state` and writes the PNG(s) to `out`.
pub fn render_snapshot(
    target: &Path,
    entry: Option<&str>,
    state: Option<&str>,
    width: u32,
    out: &Path,
) -> Result<Snap, CliError> {
    if !target.is_dir() && !target.is_file() {
        return Err(CliError::io(format!("{} does not exist", target.display())));
    }
    let st = ViewState::parse(state.unwrap_or(""));
    let frames = draw(target, entry, &st, width)?;
    let (width, height) = frames.first().map_or((0, 0), |f| f.dimensions());
    let paths = write_frames(&frames, out)?;
    Ok(Snap {
        paths,
        width,
        height,
    })
}

pub fn snapshot(args: &SnapshotArgs) -> Result<u8, CliError> {
    let snap = render_snapshot(
        &args.target,
        args.entry.as_deref(),
        args.state.as_deref(),
        args.width,
        &args.out,
    )?;
    let paths: Vec<String> = snap
        .paths
        .iter()
        .map(|p| saccade_core::paths::portable(&saccade_core::explain::absolute(p)))
        .collect();
    if args.json {
        emit(&pretty(
            &json!({"paths": paths, "width": snap.width, "height": snap.height}),
        )?)?;
    } else {
        emit(&format!("{}\n", paths.join("\n")))?;
    }
    Ok(0)
}

// ---- decision request -----------------------------------------------------

/// The decision request for `report` as JSON.
pub fn request_value(
    report: &Report,
    entries: Vec<String>,
    all_failing: bool,
    question: &str,
    intent: Option<String>,
) -> Result<Value, CliError> {
    Ok(build_request(
        report,
        &RequestOptions {
            question: Some(question_arg(question)?),
            entries,
            all_failing,
            intent,
        },
    )?)
}

pub fn decision_request(args: &DecisionRequestArgs) -> Result<u8, CliError> {
    let report = read_report(&args.report_json)?;
    let value = request_value(
        &report,
        args.entry.clone(),
        args.all_failing,
        &args.question,
        args.intent.clone(),
    )?;
    emit(&pretty(&value)?)?;
    Ok(0)
}

// ---- decide ---------------------------------------------------------------

fn field<'a>(o: &'a Map<String, Value>, k: &str) -> Option<&'a str> {
    o.get(k).and_then(Value::as_str)
}

/// Builds an [`Answer`] from a JSON object, with `defaults` filling what it omits.
fn answer_from(o: &Map<String, Value>, defaults: &DecideArgs) -> Result<Answer, CliError> {
    let entry = field(o, "entry")
        .map(str::to_owned)
        .or_else(|| defaults.entry.clone())
        .ok_or_else(|| {
            CliError::usage("the answer needs an entry (--entry or an `entry` field)")
        })?;
    let question = question_arg(field(o, "question").unwrap_or(&defaults.question))?;
    let answer = field(o, "answer")
        .map(str::to_owned)
        .or_else(|| defaults.answer.clone())
        .ok_or_else(|| CliError::usage("the answer needs an `answer`"))?;
    Ok(Answer {
        entry,
        question,
        hotspot: o
            .get("hotspot")
            .and_then(Value::as_u64)
            .map(|n| n as u32)
            .or(defaults.hotspot),
        answer,
        prob: o.get("prob").and_then(Value::as_f64).or(defaults.prob),
        confidence: o
            .get("confidence")
            .and_then(Value::as_f64)
            .or(defaults.confidence),
        source: field(o, "source")
            .map(str::to_owned)
            .or_else(|| defaults.source.clone())
            .ok_or_else(|| CliError::usage("the answer needs a source (--source)"))?,
        note: field(o, "note")
            .map(str::to_owned)
            .or_else(|| defaults.note.clone())
            .unwrap_or_default(),
        request_hash: field(o, "request_hash")
            .map(str::to_owned)
            .or_else(|| defaults.request_hash.clone()),
    })
}

fn outcome_value(a: &Answer, o: &Outcome) -> Value {
    json!({
        "schema": DECIDE_RESULT_SCHEMA,
        "entry": a.entry,
        "question": a.question.as_str(),
        "answer": a.answer,
        "source": a.source,
        "proposed": o.proposed,
        "decided": o.decided,
        "decision": o.decision.and_then(|v| serde_json::to_value(v).ok()),
        "reason": o.reason,
        "file": saccade_core::paths::portable(&saccade_core::explain::absolute(&o.file)),
    })
}

/// Records `answer` into whatever `target` names.
pub fn record(
    target: &Path,
    answer: &Answer,
    cfg: &DecisionsConfig,
    decisions_dir: Option<&Path>,
) -> Result<Outcome, CliError> {
    if target.is_dir() {
        if !target.join(VIEW_MARKER_FILE).is_file() {
            return Err(CliError::usage(format!(
                "{} is not a saccade view directory",
                target.display()
            )));
        }
        let skeleton = saccade_core::decision::view_skeleton(target)?;
        let file = target.join(saccade_core::decision::DECISIONS_FILE_NAME);
        return Ok(decide_file(&file, answer, Some(skeleton))?);
    }
    if !target.exists() {
        let id = target.to_string_lossy();
        if id.len() >= 16 && id.bytes().all(|b| b.is_ascii_hexdigit()) {
            let dir = decisions_dir.map_or_else(
                saccade_core::local::default_decisions_dir,
                Path::to_path_buf,
            );
            let file = dir.join(format!("{id}.saccade-decisions.v1.json"));
            if file.is_file() {
                return Ok(decide_file(&file, answer, None)?);
            }
            return Err(CliError::io(format!(
                "no decisions saved for session {id} in {}",
                dir.display()
            )));
        }
        return Err(CliError::io(format!("{} does not exist", target.display())));
    }
    let text = std::fs::read_to_string(target)
        .map_err(|e| CliError::io(format!("reading {}: {e}", target.display())))?;
    let schema = serde_json::from_str::<Value>(&text)
        .ok()
        .and_then(|v| v.get("schema").and_then(Value::as_str).map(str::to_owned));
    match schema.as_deref() {
        Some("saccade-report.v1") => {
            let report = read_report(target)?;
            Ok(decide_report(target, &report, cfg, answer)?)
        }
        Some("saccade-decisions.v1") => Ok(decide_file(target, answer, None)?),
        _ => Err(CliError::usage(format!(
            "{} is not a report ({REPORT_FILE_NAME}), a decisions file or a view directory",
            target.display()
        ))),
    }
}

pub fn decide(args: &DecideArgs) -> Result<u8, CliError> {
    let cfg = decisions_config(args.config.as_deref())?;
    let mut answers = Vec::new();
    if args.answer.is_some() {
        answers.push(answer_from(&Map::new(), args)?);
    } else {
        let mut text = String::new();
        std::io::stdin()
            .read_to_string(&mut text)
            .map_err(|e| CliError::io(format!("reading stdin: {e}")))?;
        for (n, line) in text.lines().filter(|l| !l.trim().is_empty()).enumerate() {
            let v: Value = serde_json::from_str(line)
                .map_err(|e| CliError::usage(format!("stdin line {}: not JSON: {e}", n + 1)))?;
            let o = v.as_object().ok_or_else(|| {
                CliError::usage(format!("stdin line {}: expected a JSON object", n + 1))
            })?;
            answers.push(answer_from(o, args)?);
        }
        if answers.is_empty() {
            return Err(CliError::usage(
                "nothing to record: pass --answer, or pipe a JSON answer on stdin",
            ));
        }
    }
    for a in &answers {
        let o = record(&args.target, a, &cfg, args.decisions_dir.as_deref())?;
        if args.json {
            emit(&format!("{}\n", outcome_value(a, &o)))?;
        } else {
            let how = match (o.decided, o.proposed) {
                (true, false) => "decided".to_owned(),
                (true, true) => "proposed and promoted by the confidence gate".to_owned(),
                (false, false) => "recorded".to_owned(),
                (false, true) => format!(
                    "proposed only ({})",
                    o.reason.as_deref().unwrap_or("not promoted")
                ),
            };
            emit(&format!(
                "{}: {} {} from {}: {how}\n",
                a.entry,
                a.question.as_str(),
                a.answer,
                a.source
            ))?;
        }
    }
    Ok(0)
}

// ---- MCP --------------------------------------------------------------------

/// The tool definitions `saccade_snapshot`, `saccade_decision_request` and `saccade_decide`.
#[cfg(feature = "mcp")]
mod bindings {
    use super::*;
    pub fn mcp_schemas() -> Vec<Value> {
        let path = |what: &str| json!({"type": "string", "minLength": 1, "description": what});
        let question = json!({"type": "string", "enum": Question::ALL.map(Question::as_str), "description": "The question type (default accept): accept = intended change or regression; triage = what kind of difference; cause = most likely cause; ask_human = does a person need to look; mask_suggest = per hotspot, noise worth masking or a real change."});
        vec![
            json!({
                "name": "saccade_snapshot",
                "title": "Render a view state to an image",
                "description": "Draws one entry of a report (or one set of a view directory) as a PNG, with no browser: the state uses the same hash grammar as the HTML pages (layout=side|swipe|flicker|heatmap, split, vertical, zoom, at=x,y, heat, channel, ev, roi=x,y,w,h, hotspot=n). Use it to look at a hotspot at zoom, a swipe at a given split or the heatmap overlay. Returns the image (at most 1600 px wide; flicker returns one image per frame) and its path.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "report_json": path("Path to saccade-report.v1.json. Give this or view_dir."),
                        "view_dir": path("A view directory written by `saccade view`. Give this or report_json."),
                        "entry": path("The entry or set name. Default: the state's set/entry, else the first failing entry."),
                        "state": {"type": "string", "description": "Hash-state string, for example `layout=swipe&split=0.3&zoom=4&at=120,80&hotspot=1`. Unknown keys are ignored."},
                        "width": {"type": "integer", "minimum": 64, "maximum": MCP_MAX_WIDTH, "description": "Stage width in pixels (default 1600)."}
                    },
                    "additionalProperties": false
                },
                "outputSchema": {"type": "object", "required": ["paths", "width", "height"], "properties": {"paths": {"type": "array", "items": {"type": "string"}}, "width": {"type": "integer"}, "height": {"type": "integer"}}},
                "annotations": {"title": "Render a view state to an image", "readOnlyHint": false, "destructiveHint": false, "idempotentHint": true, "openWorldHint": false}
            }),
            json!({
                "name": "saccade_decision_request",
                "title": "Ask bounded questions about a report",
                "description": "Read-only: emits saccade-decision-request.v1 for a report: per entry a fixed question, its allowed answers and a compact state (verdict, metrics, hotspots, flags, config differences, diagnostics, your intent). Answer from the allowed set only, then record with saccade_decide. Deterministic failures (broken or non-finite frames, config mismatches, identity breaks, errors) are listed under `skipped`: they are not questions for a model.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "report_json": path("Path to saccade-report.v1.json."),
                        "entry": {"type": "array", "items": {"type": "string"}, "description": "Entries to ask about."},
                        "all_failing": {"type": "boolean", "description": "Ask about every failing entry a model may answer."},
                        "question": question,
                        "intent": {"type": "string", "description": "What the change is for (commit message or PR text); judged against by the accept question."}
                    },
                    "required": ["report_json"],
                    "additionalProperties": false
                },
                "outputSchema": {"type": "object", "required": ["schema", "items", "skipped"], "properties": {"schema": {"const": "saccade-decision-request.v1"}, "question_type": {"type": "string"}, "items": {"type": "array", "items": {"type": "object"}}, "skipped": {"type": "array", "items": {"type": "object"}}}},
                "annotations": {"title": "Ask bounded questions about a report", "readOnlyHint": true, "destructiveHint": false, "idempotentHint": true, "openWorldHint": false}
            }),
            json!({
                "name": "saccade_decide",
                "title": "Record an answer to a decision-request question",
                "description": "Records an answer into the report's decisions file (a model's answer is a proposal that a person confirms with one click; the [decisions] confidence gate in saccade.toml may promote an accept or reject above its threshold). Refused for deterministic failures. Never changes a baseline: approve reads final decisions only.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "report_json": path("Path to saccade-report.v1.json (or a view directory, or a decisions file)."),
                        "entry": path("The entry the answer is about."),
                        "answer": {"type": "string", "description": "One of the question's allowed answers."},
                        "prob": {"type": "number", "minimum": 0, "maximum": 1, "description": "Your probability for the answer (required unless source is human)."},
                        "source": {"type": "string", "minLength": 1, "description": "Who answered: jev, openai-decisions, a model name or human."},
                        "question": question,
                        "hotspot": {"type": "integer", "minimum": 1, "description": "For mask_suggest: the hotspot the answer is about."},
                        "note": {"type": "string"},
                        "request_hash": {"type": "string", "description": "The request_hash of the item you answered."}
                    },
                    "required": ["report_json", "entry", "answer", "source"],
                    "additionalProperties": false
                },
                "outputSchema": {"type": "object", "required": ["schema", "entry", "answer", "proposed", "decided"], "properties": {"schema": {"const": "saccade-decide-result.v1"}, "entry": {"type": "string"}, "answer": {"type": "string"}, "proposed": {"type": "boolean"}, "decided": {"type": "boolean"}, "decision": {"type": ["string", "null"]}, "reason": {"type": ["string", "null"]}, "file": {"type": "string"}}},
                "annotations": {"title": "Record an answer to a decision-request question", "readOnlyHint": false, "destructiveHint": false, "idempotentHint": true, "openWorldHint": false}
            }),
        ]
    }

    /// The path-resolving callback the MCP server passes in: `(argument name, value)`
    /// to an absolute path confined to the server root.
    pub type Resolve<'a> = &'a dyn Fn(&str, &str) -> Result<PathBuf, CliError>;

    /// What an MCP tool returns: structured content, text and image blocks.
    pub type McpOutput = (Value, String, Vec<Value>);

    fn str_arg(args: &Map<String, Value>, key: &str) -> Result<Option<String>, CliError> {
        match args.get(key) {
            None | Some(Value::Null) => Ok(None),
            Some(Value::String(s)) => Ok(Some(s.clone())),
            Some(_) => Err(CliError::usage(format!("`{key}` must be a string"))),
        }
    }

    fn need(args: &Map<String, Value>, key: &str) -> Result<String, CliError> {
        str_arg(args, key)?.ok_or_else(|| CliError::usage(format!("`{key}` is required")))
    }

    fn only(args: &Map<String, Value>, known: &[&str]) -> Result<(), CliError> {
        match args.keys().find(|k| !known.contains(&k.as_str())) {
            Some(k) => Err(CliError::usage(format!(
                "unknown argument `{k}`; accepted: {}",
                known.join(", ")
            ))),
            None => Ok(()),
        }
    }

    fn fnv(s: &str) -> u32 {
        s.bytes().fold(0x811c_9dc5_u32, |h, b| {
            (h ^ u32::from(b)).wrapping_mul(0x0100_0193)
        })
    }

    fn sanitize(name: &str) -> String {
        name.chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' {
                    c
                } else {
                    '_'
                }
            })
            .collect()
    }

    fn mcp_snapshot(
        args: &Map<String, Value>,
        resolve: Resolve<'_>,
    ) -> Result<McpOutput, CliError> {
        only(
            args,
            &["report_json", "view_dir", "entry", "state", "width"],
        )?;
        let (target, dir) = match (str_arg(args, "report_json")?, str_arg(args, "view_dir")?) {
            (Some(r), None) => {
                let p = resolve("report_json", &r)?;
                let dir = p
                    .parent()
                    .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
                (p, dir)
            }
            (None, Some(v)) => {
                let p = resolve("view_dir", &v)?;
                (p.clone(), p)
            }
            _ => {
                return Err(CliError::usage(
                    "give exactly one of `report_json` and `view_dir`",
                ));
            }
        };
        let entry = str_arg(args, "entry")?;
        let state = str_arg(args, "state")?;
        let width = match args.get("width") {
            None | Some(Value::Null) => MCP_MAX_WIDTH,
            Some(v) => v
                .as_u64()
                .and_then(|w| u32::try_from(w).ok())
                .filter(|w| (64..=MCP_MAX_WIDTH).contains(w))
                .ok_or_else(|| {
                    CliError::usage(format!(
                        "`width` must be an integer from 64 to {MCP_MAX_WIDTH}"
                    ))
                })?,
        };
        let key = format!(
            "{}|{}|{width}",
            entry.as_deref().unwrap_or(""),
            state.as_deref().unwrap_or("")
        );
        let out = dir.join("snapshots").join(format!(
            "{}.{:08x}.png",
            sanitize(entry.as_deref().unwrap_or("snapshot")),
            fnv(&key)
        ));
        let snap = render_snapshot(&target, entry.as_deref(), state.as_deref(), width, &out)?;
        let mut images = Vec::new();
        for p in &snap.paths {
            let bytes = std::fs::read(p)
                .map_err(|e| CliError::io(format!("reading {}: {e}", p.display())))?;
            images.push(
            json!({"type": "image", "data": crate::mcp::base64(&bytes), "mimeType": "image/png"}),
        );
        }
        let paths: Vec<String> = snap
            .paths
            .iter()
            .map(|p| saccade_core::paths::portable(p))
            .collect();
        let text = format!(
            "saccade snapshot: {}x{}{}; {}",
            snap.width,
            snap.height,
            if paths.len() > 1 {
                format!(", {} frames", paths.len())
            } else {
                String::new()
            },
            paths.join(", ")
        );
        Ok((
            json!({"paths": paths, "width": snap.width, "height": snap.height}),
            text,
            images,
        ))
    }

    fn mcp_request(args: &Map<String, Value>, resolve: Resolve<'_>) -> Result<McpOutput, CliError> {
        only(
            args,
            &["report_json", "entry", "all_failing", "question", "intent"],
        )?;
        let path = resolve("report_json", &need(args, "report_json")?)?;
        let report = read_report(&path)?;
        let entries = match args.get("entry") {
            None | Some(Value::Null) => Vec::new(),
            Some(Value::Array(a)) => a
                .iter()
                .map(|v| {
                    v.as_str()
                        .map(str::to_owned)
                        .ok_or_else(|| CliError::usage("`entry` must be strings"))
                })
                .collect::<Result<_, _>>()?,
            Some(Value::String(s)) => vec![s.clone()],
            Some(_) => return Err(CliError::usage("`entry` must be an array of strings")),
        };
        let all = args
            .get("all_failing")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let question = str_arg(args, "question")?.unwrap_or_else(|| "accept".to_owned());
        let value = request_value(&report, entries, all, &question, str_arg(args, "intent")?)?;
        let n = value["items"].as_array().map_or(0, Vec::len);
        let text = format!(
            "saccade decision request: {n} item(s), question {question}; answer each from `allowed_answers`, then call saccade_decide with the item's `request_hash`.\n{value}"
        );
        Ok((value, text, Vec::new()))
    }

    fn mcp_decide(args: &Map<String, Value>, resolve: Resolve<'_>) -> Result<McpOutput, CliError> {
        only(
            args,
            &[
                "report_json",
                "entry",
                "answer",
                "prob",
                "source",
                "question",
                "hotspot",
                "note",
                "request_hash",
            ],
        )?;
        let target = resolve("report_json", &need(args, "report_json")?)?;
        let defaults = DecideArgs {
            target: target.clone(),
            entry: None,
            question: "accept".into(),
            hotspot: None,
            answer: None,
            prob: None,
            confidence: None,
            source: None,
            note: None,
            request_hash: None,
            config: None,
            decisions_dir: None,
            json: true,
        };
        let answer = answer_from(args, &defaults)?;
        let cfg_path = resolve("config", "saccade.toml")?;
        let cfg = decisions_config(
            Some(&cfg_path)
                .filter(|p| p.is_file())
                .map(PathBuf::as_path),
        )?;
        let outcome = record(&target, &answer, &cfg, None)?;
        let value = outcome_value(&answer, &outcome);
        let text = format!(
            "saccade decide: {} {} from {} recorded{}",
            answer.entry,
            answer.answer,
            answer.source,
            if outcome.decided {
                " and decided"
            } else if outcome.proposed {
                " as a proposal"
            } else {
                ""
            }
        );
        Ok((value, text, Vec::new()))
    }

    /// Runs the MCP tool `name`, or `None` when it is not one of these.
    pub fn mcp_call(
        name: &str,
        args: &Map<String, Value>,
        resolve: Resolve<'_>,
    ) -> Option<Result<McpOutput, CliError>> {
        Some(match name {
            "saccade_snapshot" => mcp_snapshot(args, resolve),
            "saccade_decision_request" => mcp_request(args, resolve),
            "saccade_decide" => mcp_decide(args, resolve),
            _ => return None,
        })
    }
}
#[cfg(feature = "mcp")]
pub use bindings::{mcp_call, mcp_schemas};
