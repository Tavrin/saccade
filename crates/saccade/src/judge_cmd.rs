//! `saccade judge`: panels of decision models and humans judge a report's
//! entries or the candidates of a ranking, with calibration and self-tests.
//! Shared by the CLI and the MCP tools `saccade_judge` and
//! `saccade_judge_calibrate`.

use std::path::{Path, PathBuf};
use std::time::Duration;

use clap::{Args, Subcommand};
use saccade_core::config::RunConfig;
use saccade_core::decision::DecisionsConfig;
use saccade_core::judge::{
    EvidenceOptions, JUDGE_FILE_NAME, JudgeQuestion, Panel, RunOptions, dry_run_value, execute,
    make_plan, rank_items, record, report_items,
};
#[cfg(feature = "evaluation")]
use saccade_core::judge::{SelftestOptions, selftest};
use saccade_core::judge_provider::{Keys, LiveBackend, Retry};
use saccade_core::judge_stats::CALIBRATION_SCHEMA;
#[cfg(feature = "evaluation")]
use saccade_core::judge_stats::{CalibrateOptions, calibrate};
use saccade_core::rank::{RANK_SCHEMA, RankReport};
use saccade_core::report::{REPORT_SCHEMA, Report};
#[cfg(feature = "mcp")]
use serde_json::Map;
use serde_json::{Value, json};

use crate::agent::CliError;

/// `saccade judge`.
#[derive(Args)]
#[command(args_conflicts_with_subcommands = true)]
pub struct JudgeArgs {
    /// `calibrate` or `selftest`; without one, run a panel.
    #[command(subcommand)]
    pub sub: Option<JudgeSub>,
    #[command(flatten)]
    pub run: RunArgs,
}

/// The `judge` subcommands.
#[derive(Subcommand)]
pub enum JudgeSub {
    /// Measure each judge against human labels and write `saccade-calibration.v1`.
    #[cfg(feature = "evaluation")]
    Calibrate(CalibrateArgs),
    /// Re-ask with irrelevant perturbations and report the answer flip rates.
    #[cfg(feature = "evaluation")]
    Selftest(SelftestArgs),
    /// Benchmark the pinned model chain and Jev against human finals.
    #[cfg(feature = "evaluation")]
    Bench(crate::review_cmd::BenchArgs),
    /// Harvest evidence-bound human final labels.
    #[cfg(feature = "evaluation")]
    CollectLabels(crate::review_cmd::CollectArgs),
}

/// Arguments of a panel run.
#[derive(Args)]
pub struct RunArgs {
    /// A `saccade-report.v1.json` or a `saccade-rank.v1.json`.
    pub target: Option<PathBuf>,
    /// The panel file: an optional `[panel]` table and `[[judge]]` entries.
    #[arg(long)]
    pub panel: Option<PathBuf>,
    /// Question: accept, triage, cause, ask_human, mask_suggest or preference
    /// (preference is the only question a ranking takes).
    #[arg(long, default_value = "accept")]
    pub question: String,
    /// What the change is for, or the preference criterion.
    #[arg(long)]
    pub intent: Option<String>,
    /// Judge this entry only (repeatable); default every failing entry.
    #[arg(long)]
    pub entry: Vec<String>,
    /// Ask text judges in both presentation orders too (vision judges, human
    /// voters and preferences always are).
    #[arg(long)]
    pub both_orders: bool,
    /// Print the requests that would be made and call no one.
    #[arg(long)]
    pub dry_run: bool,
    /// Directory holding `jev.env` and `gemini.env` (default `~/.config/saccade`).
    #[arg(long, value_name = "DIR")]
    pub keys_dir: Option<PathBuf>,
    /// External OCR command for the text diff of screenshots; `{image}` stands for
    /// the image path. Its output goes to the judges; see the README's privacy note.
    #[arg(long, value_name = "CMD")]
    pub ocr_cmd: Option<String>,
    /// Canaries mixed in, as a fraction of the real items (overrides the panel).
    #[arg(long)]
    pub canary_rate: Option<f64>,
    /// Most calls the run may make.
    #[arg(long, default_value_t = 100)]
    pub max_calls: usize,
    /// For a ranking: most pairwise items to judge.
    #[arg(long, default_value_t = 12)]
    pub max_pairs: usize,
    /// A `saccade-calibration.v1` file to read each judge's record from.
    #[arg(long)]
    pub calibration: Option<PathBuf>,
    /// Where `saccade serve` keeps decisions (human votes live under `judge/`).
    #[arg(long)]
    pub decisions_dir: Option<PathBuf>,
    /// Measurement config (default `./saccade.toml` when present); no approval gate.
    #[arg(long)]
    pub config: Option<PathBuf>,
    /// Where to write the result (default `saccade-judge.v1.json` next to the report).
    #[arg(long)]
    pub out: Option<PathBuf>,
    /// Do not record proposals into the report's decisions file.
    #[arg(long)]
    pub no_record: bool,
}

/// Arguments of `judge calibrate`.
#[derive(Args)]
#[cfg(feature = "evaluation")]
pub struct CalibrateArgs {
    /// Decisions files with human final decisions (and the judges' proposals).
    #[arg(long, required = true, num_args = 1..)]
    pub labels: Vec<PathBuf>,
    /// `saccade-judge.v1` result files, for each judge's position bias.
    #[arg(long, num_args = 1..)]
    pub runs: Vec<PathBuf>,
    /// Where to write `saccade-calibration.v1`.
    #[arg(long, default_value = "saccade-calibration.v1.json")]
    pub out: PathBuf,
    /// Accuracy a suggested gate threshold must reach.
    #[arg(long, default_value_t = 0.95)]
    pub target_accuracy: f64,
    /// Fewest predictions at or above a suggested threshold.
    #[arg(long, default_value_t = 10)]
    pub min_support: usize,
}

/// Arguments of `judge selftest`.
#[derive(Args)]
#[cfg(feature = "evaluation")]
pub struct SelftestArgs {
    /// A `saccade-report.v1.json`.
    pub target: PathBuf,
    /// The panel file.
    #[arg(long)]
    pub panel: PathBuf,
    /// The question to re-ask.
    #[arg(long, default_value = "accept")]
    pub question: String,
    /// What the change is for.
    #[arg(long)]
    pub intent: Option<String>,
    /// Items re-asked per judge (each costs five calls per judge).
    #[arg(long, default_value_t = 3)]
    pub items: usize,
    /// Pixels the crop is shifted by.
    #[arg(long, default_value_t = 7)]
    pub offset: u32,
    /// Most calls the self-test may make.
    #[arg(long, default_value_t = 60)]
    pub max_calls: usize,
    /// Print the calls that would be made and call no one.
    #[arg(long)]
    pub dry_run: bool,
    /// Directory holding the key files.
    #[arg(long, value_name = "DIR")]
    pub keys_dir: Option<PathBuf>,
    /// Where to write the result (default: not written).
    #[arg(long)]
    pub out: Option<PathBuf>,
}

fn question_of(name: &str) -> Result<JudgeQuestion, CliError> {
    JudgeQuestion::parse(name).ok_or_else(|| {
        CliError::usage(format!(
            "unknown question {name:?}; one of: {}",
            JudgeQuestion::NAMES.join(", ")
        ))
    })
}

fn read_json(path: &Path) -> Result<Value, CliError> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| CliError::io(format!("reading {}: {e}", path.display())))?;
    serde_json::from_str(&text)
        .map_err(|e| CliError::io(format!("parsing {}: {e}", path.display())))
}

fn write_json(path: &Path, v: &Value) -> Result<(), CliError> {
    let text = format!("{}\n", serde_json::to_string_pretty(v)?);
    std::fs::write(path, text).map_err(|e| CliError::io(format!("writing {}: {e}", path.display())))
}

fn gate_config(path: Option<&Path>) -> Result<DecisionsConfig, CliError> {
    let path = match path {
        Some(p) => Some(p),
        None => Some(Path::new("saccade.toml")).filter(|p| p.is_file()),
    };
    Ok(match path {
        Some(p) => RunConfig::from_toml_file(p)?.decisions,
        None => DecisionsConfig::default(),
    })
}

fn backend(keys_dir: Option<PathBuf>, panel: &Panel) -> LiveBackend {
    LiveBackend::new(
        Keys::new(keys_dir),
        Retry {
            max_retries: panel.settings.max_retries,
            ..Retry::default()
        },
        Duration::from_secs(150),
    )
}

/// The inputs of one panel run, from the CLI or an MCP call.
pub struct Job {
    /// The report or ranking.
    pub target: PathBuf,
    /// The panel file.
    pub panel: PathBuf,
    /// Question name.
    pub question: String,
    /// Intent or criterion.
    pub intent: Option<String>,
    /// Entries to judge; empty is every failing one.
    pub entries: Vec<String>,
    /// Both orders for text judges.
    pub both_orders: bool,
    /// Plan only.
    pub dry_run: bool,
    /// Key directory.
    pub keys_dir: Option<PathBuf>,
    /// OCR command.
    pub ocr_cmd: Option<String>,
    /// Canary rate override.
    pub canary_rate: Option<f64>,
    /// Call budget.
    pub max_calls: usize,
    /// Pairwise item budget for a ranking.
    pub max_pairs: usize,
    /// Calibration file.
    pub calibration: Option<PathBuf>,
    /// Decisions directory.
    pub decisions_dir: Option<PathBuf>,
    /// Measurement config; it cannot grant approval authority.
    pub config: Option<PathBuf>,
    /// Result file.
    pub out: Option<PathBuf>,
    /// Record proposals.
    pub record: bool,
    /// MCP confinement boundary for every nested report/image path.
    pub read_root: Option<PathBuf>,
}

impl From<RunArgs> for Result<Job, CliError> {
    fn from(a: RunArgs) -> Self {
        Ok(Job {
            target: a.target.ok_or_else(|| {
                CliError::usage(
                    "name a report or ranking JSON to judge (see `saccade judge --help`)",
                )
            })?,
            panel: a
                .panel
                .ok_or_else(|| CliError::usage("--panel <panel.toml> is required"))?,
            question: a.question,
            intent: a.intent,
            entries: a.entry,
            both_orders: a.both_orders,
            dry_run: a.dry_run,
            keys_dir: a.keys_dir,
            ocr_cmd: a.ocr_cmd,
            canary_rate: a.canary_rate,
            max_calls: a.max_calls,
            max_pairs: a.max_pairs,
            calibration: a.calibration,
            decisions_dir: a.decisions_dir,
            config: a.config,
            out: a.out,
            record: !a.no_record,
            read_root: None,
        })
    }
}

/// Runs a panel and returns the `saccade-judge.v1` document and a short text summary.
pub fn run(job: &Job) -> Result<(Value, String), CliError> {
    let panel = Panel::from_file(&job.panel)?;
    let question = question_of(&job.question)?;
    let doc = read_json(&job.target)?;
    let dir = job
        .target
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
    if let Some(root) = &job.read_root {
        confined_document(&doc, &dir, root)?;
    }
    if job
        .canary_rate
        .is_some_and(|r| !r.is_finite() || !(0.0..=1.0).contains(&r))
    {
        return Err(CliError::usage("canary-rate must be in [0, 1]"));
    }
    let ev = EvidenceOptions {
        ocr_cmd: job.ocr_cmd.clone(),
    };
    let calibration = match &job.calibration {
        Some(p) => {
            let v = read_json(p)?;
            if v["schema"] != CALIBRATION_SCHEMA {
                return Err(CliError::usage(format!(
                    "{} is not a {CALIBRATION_SCHEMA} file",
                    p.display()
                )));
            }
            Some(v)
        }
        None => None,
    };
    let opts = RunOptions {
        question,
        both_orders: job.both_orders,
        canary_rate: job.canary_rate,
        max_calls: job.max_calls,
        decisions_dir: Some(
            job.decisions_dir
                .clone()
                .unwrap_or_else(saccade_core::local::default_decisions_dir),
        ),
        calibration,
    };
    let mut report: Option<Report> = None;
    let mut labels: Vec<String> = Vec::new();
    let (items, skipped) = match doc["schema"].as_str().map(saccade_core::report_links::original_schema) {
        Some(REPORT_SCHEMA) => {
            let r: Report = serde_json::from_value(doc)?;
            let out = report_items(&r, &dir, question, job.intent.as_deref(), &job.entries, &ev)?;
            report = Some(r);
            out
        }
        Some(RANK_SCHEMA) => {
            if question != JudgeQuestion::Preference {
                return Err(CliError::usage(
                    "a ranking is judged with --question preference (pairwise votes between its candidates)",
                ));
            }
            let r: RankReport = serde_json::from_value(doc)?;
            let (items, l) = rank_items(&r, job.intent.as_deref(), job.max_pairs)?;
            labels = l;
            (items, Vec::new())
        }
        _ => {
            return Err(CliError::usage(format!(
                "{} is neither a {REPORT_SCHEMA} nor a {RANK_SCHEMA} file",
                job.target.display()
            )));
        }
    };
    if items.is_empty() {
        return Err(CliError::usage(
            "nothing to judge: no failing entry a model may answer (name one with --entry, or see `skipped`)",
        ));
    }
    let plan = make_plan(items, skipped, &panel, &opts);
    if job.dry_run {
        let v = dry_run_value(&plan, &panel, &opts);
        let n = v["calls_planned"].as_u64().unwrap_or(0);
        return Ok((v, format!("dry run: {n} request(s) planned, none made")));
    }
    let live = backend(job.keys_dir.clone(), &panel);
    let result = execute(&plan, &panel, &live, &opts);
    let mut value = result.value.clone();
    if let Some(cands) = value["ranking"]["candidates"].as_array_mut() {
        for (c, label) in cands.iter_mut().zip(&labels) {
            c["label"] = json!(label);
        }
    }
    if let (true, Some(r)) = (job.record, &report) {
        let cfg = gate_config(job.config.as_deref())?;
        let rows = record(&result, &job.target, r, &cfg);
        value["recorded"] = Value::Array(rows);
    }
    let out = job.out.clone().unwrap_or_else(|| dir.join(JUDGE_FILE_NAME));
    write_json(&out, &value)?;
    value["result_file"] = json!(saccade_core::explain::absolute(&out).display().to_string());
    Ok((value.clone(), summary(&value)))
}

pub(crate) fn confined_document(doc: &Value, dir: &Path, root: &Path) -> Result<(), CliError> {
    let check = |p: PathBuf| -> Result<PathBuf, CliError> {
        let canon = saccade_core::paths::canonicalize(&p)
            .map_err(|e| CliError::io(format!("{}: {e}", p.display())))?;
        if !canon.starts_with(root) {
            return Err(CliError::usage(
                "a nested judge input path escapes the MCP root",
            ));
        }
        Ok(canon)
    };
    if doc["schema"] == RANK_SCHEMA {
        for c in doc["overall"].as_array().into_iter().flatten() {
            if let Some(p) = c["report_json"].as_str() {
                let p = check(PathBuf::from(p))?;
                confined_document(&read_json(&p)?, p.parent().unwrap_or(dir), root)?;
            }
        }
    } else {
        for e in doc["entries"].as_array().into_iter().flatten() {
            for side in ["baseline", "capture"] {
                if let Some(p) = e["paths"][side].as_str() {
                    check(dir.join(p))?;
                }
            }
        }
    }
    Ok(())
}

fn summary(v: &Value) -> String {
    let mut s = String::new();
    for it in v["items"].as_array().into_iter().flatten() {
        let r = &it["result"];
        s.push_str(&format!(
            "{} [{}]: {} ({}; agreement {:.2}, probability {:.2})\n",
            it["id"].as_str().unwrap_or(""),
            it["question"].as_str().unwrap_or(""),
            r["answer"].as_str().unwrap_or("-"),
            r["status"].as_str().unwrap_or(""),
            r["agreement"].as_f64().unwrap_or(0.0),
            r["prob"].as_f64().unwrap_or(0.0),
        ));
    }
    for c in v["canaries"]["judges"].as_array().into_iter().flatten() {
        if c["flagged"] == true {
            s.push_str(&format!(
                "canary failure: {} (accuracy {:.2}), weight x{:.2}\n",
                c["judge"].as_str().unwrap_or(""),
                c["accuracy"].as_f64().unwrap_or(0.0),
                c["weight_factor"].as_f64().unwrap_or(1.0)
            ));
        }
    }
    if let Some(p) = v["human_votes"]["path"].as_str() {
        s.push_str(&format!(
            "human votes: open {p} on a running `saccade serve`, then run this command again\n"
        ));
    }
    s
}

/// `saccade judge calibrate`.
#[cfg(feature = "evaluation")]
pub fn run_calibrate(
    labels: &[PathBuf],
    runs: &[PathBuf],
    out: &Path,
    opts: &CalibrateOptions,
) -> Result<Value, CliError> {
    let v = calibrate(labels, runs, opts)?;
    write_json(out, &v)?;
    Ok(v)
}

#[cfg(feature = "evaluation")]
fn selftest_run(a: &SelftestArgs) -> Result<Value, CliError> {
    let panel = Panel::from_file(&a.panel)?;
    let question = question_of(&a.question)?;
    let r: Report = serde_json::from_value(read_json(&a.target)?)?;
    let dir = a
        .target
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
    let (items, _) = report_items(
        &r,
        &dir,
        question,
        a.intent.as_deref(),
        &[],
        &EvidenceOptions::default(),
    )?;
    let live = backend(a.keys_dir.clone(), &panel);
    let v = selftest(
        &items,
        &panel,
        &live,
        &SelftestOptions {
            items: a.items,
            offset_px: a.offset,
            max_calls: a.max_calls,
        },
        a.dry_run,
    );
    if let Some(out) = &a.out {
        write_json(out, &v)?;
    }
    Ok(v)
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

/// Entry point of `saccade judge`.
pub fn judge(args: JudgeArgs) -> Result<u8, CliError> {
    eprintln!(
        "judge mode is experimental: answers are proposals; validate against your own decisions with `saccade judge calibrate`"
    );
    match args.sub {
        #[cfg(feature = "evaluation")]
        Some(JudgeSub::Bench(b)) => {
            let v = crate::review_cmd::bench(b, None)?;
            emit(&format!("{}\n", serde_json::to_string_pretty(&v)?))?;
        }
        #[cfg(feature = "evaluation")]
        Some(JudgeSub::CollectLabels(c)) => {
            let v = crate::review_cmd::collect(c)?;
            emit(&format!("{}\n", serde_json::to_string_pretty(&v)?))?;
        }
        #[cfg(feature = "evaluation")]
        Some(JudgeSub::Calibrate(c)) => {
            let v = run_calibrate(
                &c.labels,
                &c.runs,
                &c.out,
                &CalibrateOptions {
                    target_accuracy: c.target_accuracy,
                    min_support: c.min_support,
                },
            )?;
            emit(&format!("{}\n", serde_json::to_string_pretty(&v)?))?;
            eprintln!("calibration written to {}", c.out.display());
        }
        #[cfg(feature = "evaluation")]
        Some(JudgeSub::Selftest(s)) => {
            let v = selftest_run(&s)?;
            emit(&format!("{}\n", serde_json::to_string_pretty(&v)?))?;
        }
        None => {
            let job: Job = Result::<Job, CliError>::from(args.run)?;
            let (v, text) = run(&job)?;
            emit(&format!("{}\n", serde_json::to_string_pretty(&v)?))?;
            eprint!("{text}");
            if !text.ends_with('\n') {
                eprintln!();
            }
        }
    }
    Ok(0)
}

// ---- MCP ------------------------------------------------------------------

/// The tool definitions `saccade_judge` and `saccade_judge_calibrate`.
#[cfg(feature = "mcp")]
mod bindings {
    use super::*;
    pub fn mcp_schemas() -> Vec<Value> {
        let path = |what: &str| json!({"type": "string", "minLength": 1, "description": what});
        vec![
            json!({
                "name": "saccade_judge",
                "title": "Judge a report or ranking with a panel",
                "description": "Experimental judge mode: answers are proposals; validate against your own decisions with `saccade judge calibrate`. Asks a panel of decision models (and, through `saccade serve`, people) a bounded question per failing entry, or pairwise preferences between the candidates of a ranking. Every question is typed (checkable, rubric or preference) and the result states what that kind of answer can and cannot tell you; text-only judges get an evidence encoding, never pixels, and vision judges only blind hotspot strips. Answers are recorded as proposals; disagreement or low confidence escalates to needs_human. Sends evidence to the panel's providers: use only data you may share. Set dry_run to see the requests without making any.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "target": path("Path to saccade-report.v1.json or saccade-rank.v1.json."),
                        "panel": path("Path to the panel TOML file."),
                        "question": {"type": "string", "enum": JudgeQuestion::NAMES, "description": "Question type (default accept); a ranking takes preference."},
                        "intent": {"type": "string", "description": "What the change is for, or the preference criterion."},
                        "entry": {"type": "array", "items": {"type": "string"}, "description": "Entries to judge (default: every failing one)."},
                        "both_orders": {"type": "boolean", "description": "Ask text judges in both presentation orders as well."},
                        "dry_run": {"type": "boolean", "description": "Return the requests that would be made and call no one."},
                        "canary_rate": {"type": "number", "minimum": 0, "maximum": 1},
                        "max_calls": {"type": "integer", "minimum": 1, "maximum": 1000},
                        "max_pairs": {"type": "integer", "minimum": 1, "maximum": 200, "description": "For a ranking: most pairwise items."},
                        "calibration": path("A saccade-calibration.v1 file."),
                        "record": {"type": "boolean", "description": "Record proposals into the decisions file (default true)."}
                    },
                    "required": ["target", "panel"],
                    "additionalProperties": false
                },
                "outputSchema": {"type": "object", "required": ["schema"], "properties": {"schema": {"const": "saccade-judge.v1"}, "run_id": {"type": "string"}, "dry_run": {"type": "boolean"}, "items": {"type": "array", "items": {"type": "object"}}, "requests": {"type": "array", "items": {"type": "object"}}}},
                "annotations": {"title": "Judge a report or ranking with a panel", "readOnlyHint": false, "destructiveHint": false, "idempotentHint": false, "openWorldHint": true}
            }),
            json!({
                "name": "saccade_judge_calibrate",
                "title": "Calibrate judges against human labels",
                "description": "Experimental judge mode: evaluate proposal answers against historical human labels; calibration never grants baseline approval authority. Measures each judge per question type against human final decisions: accuracy, agreement with humans, expected calibration error with a reliability table, position bias and the human-human ceiling (Krippendorff's alpha), and suggests gate thresholds. Writes and returns saccade-calibration.v1. Reads local files only.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "labels": {"type": "array", "minItems": 1, "items": {"type": "string"}, "description": "Decisions files with human final decisions."},
                        "runs": {"type": "array", "items": {"type": "string"}, "description": "saccade-judge.v1 result files, for position bias."},
                        "out": path("Where to write the calibration (default saccade-calibration.v1.json under the server root)."),
                        "target_accuracy": {"type": "number", "minimum": 0, "maximum": 1},
                        "min_support": {"type": "integer", "minimum": 1}
                    },
                    "required": ["labels"],
                    "additionalProperties": false
                },
                "outputSchema": {"type": "object", "required": ["schema", "judges"], "properties": {"schema": {"const": "saccade-calibration.v1"}, "judges": {"type": "array", "items": {"type": "object"}}, "human_ceiling": {"type": "array", "items": {"type": "object"}}}},
                "annotations": {"title": "Calibrate judges against human labels", "readOnlyHint": false, "destructiveHint": false, "idempotentHint": true, "openWorldHint": false}
            }),
        ]
    }

    type Resolve<'a> = &'a dyn Fn(&str, &str) -> Result<PathBuf, CliError>;

    fn str_arg(args: &Map<String, Value>, key: &str) -> Result<Option<String>, CliError> {
        match args.get(key) {
            None | Some(Value::Null) => Ok(None),
            Some(Value::String(s)) => Ok(Some(s.clone())),
            Some(_) => Err(CliError::usage(format!("`{key}` must be a string"))),
        }
    }

    fn strings(args: &Map<String, Value>, key: &str) -> Result<Vec<String>, CliError> {
        match args.get(key) {
            None | Some(Value::Null) => Ok(Vec::new()),
            Some(Value::Array(a)) => a
                .iter()
                .map(|v| {
                    v.as_str()
                        .map(str::to_owned)
                        .ok_or_else(|| CliError::usage(format!("`{key}` must be strings")))
                })
                .collect(),
            Some(_) => Err(CliError::usage(format!(
                "`{key}` must be an array of strings"
            ))),
        }
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

    fn flag(args: &Map<String, Value>, key: &str) -> bool {
        args.get(key).and_then(Value::as_bool).unwrap_or(false)
    }

    fn number(args: &Map<String, Value>, key: &str) -> Option<f64> {
        args.get(key).and_then(Value::as_f64)
    }

    /// Runs the MCP tool `name`, or `None` when it is not a judge tool.
    pub fn mcp_call(
        name: &str,
        args: &Map<String, Value>,
        resolve: Resolve<'_>,
    ) -> Option<Result<(Value, String), CliError>> {
        match name {
            "saccade_judge" => Some(mcp_judge(args, resolve)),
            #[cfg(feature = "evaluation")]
            "saccade_judge_calibrate" => Some(mcp_calibrate(args, resolve)),
            _ => None,
        }
    }

    fn mcp_judge(
        args: &Map<String, Value>,
        resolve: Resolve<'_>,
    ) -> Result<(Value, String), CliError> {
        only(
            args,
            &[
                "target",
                "panel",
                "question",
                "intent",
                "entry",
                "both_orders",
                "dry_run",
                "canary_rate",
                "max_calls",
                "max_pairs",
                "calibration",
                "record",
            ],
        )?;
        let need = |k: &str| -> Result<PathBuf, CliError> {
            let v =
                str_arg(args, k)?.ok_or_else(|| CliError::usage(format!("`{k}` is required")))?;
            resolve(k, &v)
        };
        let calibration = match str_arg(args, "calibration")? {
            Some(c) => Some(resolve("calibration", &c)?),
            None => None,
        };
        let job = Job {
            target: need("target")?,
            panel: need("panel")?,
            question: str_arg(args, "question")?.unwrap_or_else(|| "accept".into()),
            intent: str_arg(args, "intent")?,
            entries: strings(args, "entry")?,
            both_orders: flag(args, "both_orders"),
            dry_run: flag(args, "dry_run"),
            keys_dir: None,
            ocr_cmd: None,
            canary_rate: number(args, "canary_rate"),
            max_calls: number(args, "max_calls").map_or(100, |n| n as usize),
            max_pairs: number(args, "max_pairs").map_or(12, |n| n as usize),
            calibration,
            decisions_dir: Some(resolve("decisions_dir", "judge-decisions")?),
            config: {
                let c = resolve("config", "saccade.toml")?;
                c.is_file().then_some(c)
            },
            out: None,
            record: args.get("record").and_then(Value::as_bool).unwrap_or(true),
            read_root: Some(
                saccade_core::paths::canonicalize(resolve("root", ".")?)
                    .map_err(|e| CliError::io(format!("resolving the MCP root: {e}")))?,
            ),
        };
        run(&job)
    }

    #[cfg(feature = "evaluation")]
    fn mcp_calibrate(
        args: &Map<String, Value>,
        resolve: Resolve<'_>,
    ) -> Result<(Value, String), CliError> {
        only(
            args,
            &["labels", "runs", "out", "target_accuracy", "min_support"],
        )?;
        let paths = |k: &str| -> Result<Vec<PathBuf>, CliError> {
            strings(args, k)?.iter().map(|p| resolve(k, p)).collect()
        };
        let labels = paths("labels")?;
        if labels.is_empty() {
            return Err(CliError::usage(
                "`labels` needs at least one decisions file",
            ));
        }
        let out = resolve(
            "out",
            &str_arg(args, "out")?.unwrap_or_else(|| "saccade-calibration.v1.json".into()),
        )?;
        let opts = CalibrateOptions {
            target_accuracy: number(args, "target_accuracy").unwrap_or(0.95),
            min_support: number(args, "min_support").map_or(10, |n| n as usize),
        };
        let v = run_calibrate(&labels, &paths("runs")?, &out, &opts)?;
        let text = format!(
            "saccade judge calibrate: {} judge row(s) from {} labelled unit(s); written to {}",
            v["judges"].as_array().map_or(0, Vec::len),
            v["labelled_units"],
            out.display()
        );
        Ok((v, text))
    }
}
#[cfg(feature = "mcp")]
pub use bindings::{mcp_call, mcp_schemas};
