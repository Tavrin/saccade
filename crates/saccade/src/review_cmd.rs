//! CLI and MCP surfaces for the review cascade and label benchmark.
use crate::agent::CliError;
use clap::Args;
use saccade_core::judge_provider::{Keys, LiveBackend, Retry};
use saccade_core::review::{self, Options, Profile};
#[cfg(feature = "mcp")]
use serde_json::Map;
use serde_json::Value;
#[cfg(feature = "mcp")]
use serde_json::json;
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Args)]
pub(crate) struct ReviewArgs {
    /// Existing comparison report.
    pub report_json: PathBuf,
    #[arg(long, default_value = "nightly")]
    pub profile: String,
    #[arg(long, conflicts_with = "intent_file")]
    pub intent: Option<String>,
    #[arg(long)]
    pub intent_file: Option<PathBuf>,
    #[arg(long, default_value_t = 30)]
    pub budget_calls: usize,
    #[arg(long)]
    pub max_gemini: Option<usize>,
    #[arg(long)]
    pub json: bool,
    #[arg(long)]
    pub dry_run: bool,
    #[arg(long)]
    pub keys_dir: Option<PathBuf>,
    #[arg(long)]
    pub cache_dir: Option<PathBuf>,
    #[arg(long)]
    pub decisions_dir: Option<PathBuf>,
    /// Root passed to saccade serve, used for exact local deep links.
    #[arg(long)]
    pub serve_root: Option<PathBuf>,
    #[arg(long)]
    pub ocr_cmd: Option<String>,
}

#[derive(Args)]
#[cfg(feature = "evaluation")]
pub(crate) struct BenchArgs {
    #[arg(long)]
    pub labels: PathBuf,
    #[arg(long,value_delimiter=',',num_args=1..)]
    pub models: Vec<String>,
    #[arg(long,value_delimiter=',',num_args=1..)]
    pub questions: Vec<String>,
    #[arg(long, default_value_t = 30)]
    pub budget_calls: usize,
    #[arg(long)]
    pub keys_dir: Option<PathBuf>,
    #[arg(long)]
    pub cache_dir: Option<PathBuf>,
    #[arg(long, default_value = "saccade-judge-bench.v1.json")]
    pub out: PathBuf,
    /// Provider-specific HTTP cap, including retries (use remaining live allowance).
    #[arg(long, default_value_t = 15)]
    pub jev_call_limit: usize,
    /// Provider-specific HTTP cap, including retries.
    #[arg(long, default_value_t = 30)]
    pub gemini_call_limit: usize,
}
#[derive(Args)]
#[cfg(feature = "evaluation")]
pub(crate) struct CollectArgs {
    #[arg(long)]
    pub decisions_dir: Option<PathBuf>,
    #[arg(long,num_args=1..)]
    pub reports: Vec<PathBuf>,
    #[arg(long)]
    pub out: PathBuf,
}

fn read(path: &Path) -> Result<Value, CliError> {
    let bytes = std::fs::read(path)
        .map_err(|e| CliError::io(format!("reading {}: {e}", path.display())))?;
    Ok(serde_json::from_slice(&bytes)?)
}
fn live(
    keys: Option<PathBuf>,
    cache: Option<PathBuf>,
    profile: &Profile,
    budget: usize,
) -> LiveBackend {
    LiveBackend::new(
        Keys::new(keys),
        Retry {
            max_retries: profile.max_retries,
            base_ms: 500,
        },
        Duration::from_secs(60),
    )
    .with_policy(
        cache.unwrap_or_else(saccade_core::local::default_cache_dir),
        profile.cooldown_secs,
        budget,
        budget,
        budget,
    )
}
fn root(path: Option<&Path>) -> Result<PathBuf, CliError> {
    let p = path
        .map(Path::to_path_buf)
        .unwrap_or(std::env::current_dir().map_err(|e| CliError::io(e.to_string()))?);
    saccade_core::paths::canonicalize(p)
        .map_err(|e| CliError::io(format!("resolving serve root: {e}")))
}

fn review_run(args: &ReviewArgs, read_root: Option<&Path>) -> Result<(Value, String), CliError> {
    let document = saccade_core::paths::canonicalize(&args.report_json)
        .map_err(|e| CliError::io(e.to_string()))?;
    let doc = read(&document)?;
    if doc["schema"] != saccade_core::report::REPORT_SCHEMA {
        return Err(CliError::usage("review needs a saccade-report.v1 document"));
    }
    if let Some(r) = read_root {
        crate::judge_cmd::confined_document(&doc, document.parent().unwrap_or(Path::new(".")), r)?;
    }
    let report = serde_json::from_value(doc)?;
    let profile = Profile::load(&args.profile)?;
    if read_root.is_some() && profile.ocr_cmd.is_some() {
        return Err(CliError::usage(
            "configured OCR execution is CLI-only; use a profile without ocr_cmd for MCP",
        ));
    }
    let intent = match &args.intent_file {
        Some(p) => Some(
            std::fs::read_to_string(p).map_err(|e| CliError::io(format!("reading intent: {e}")))?,
        ),
        None => args.intent.clone(),
    };
    let backend = live(
        args.keys_dir.clone(),
        args.cache_dir.clone(),
        &profile,
        args.budget_calls,
    );
    let opts = Options {
        profile,
        intent,
        budget_calls: args.budget_calls,
        max_gemini: args.max_gemini,
        dry_run: args.dry_run,
        decisions_dir: args
            .decisions_dir
            .clone()
            .unwrap_or_else(saccade_core::local::default_decisions_dir),
        serve_root: root(args.serve_root.as_deref())?,
        ocr_cmd: args.ocr_cmd.clone(),
    };
    let value = review::run(&document, &report, &backend, &opts)?;
    let text = review::text(&value);
    if !args.dry_run {
        let dir = document.parent().unwrap_or(Path::new("."));
        review::write_json(&dir.join(review::FILE), &value)?;
        report_sections(dir, &text)?;
    }
    Ok((value, text))
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
fn report_sections(dir: &Path, text: &str) -> Result<(), CliError> {
    let html = dir.join("index.html");
    if html.is_file() {
        let mut content =
            std::fs::read_to_string(&html).map_err(|e| CliError::io(e.to_string()))?;
        const START: &str = "<!-- saccade-review:start -->";
        const END: &str = "<!-- saccade-review:end -->";
        if let Some(start) = content.find(START)
            && let Some(end) = content[start..].find(END)
        {
            content.replace_range(start..start + end + END.len(), "");
        }
        let block = format!(
            "{START}<section aria-label=\"AI review\"><h2>AI review (experimental)</h2><p>Proposals only. Human approval required.</p><pre>{}</pre></section>{END}",
            escape(text)
        );
        content = content.replacen("<main>", &format!("<main>{block}"), 1);
        std::fs::write(html, content).map_err(|e| CliError::io(e.to_string()))?;
    }
    let md = dir.join("saccade-review.md");
    std::fs::write(&md,format!("### AI review (experimental)\n\nProposals only. Never approve on model output.\n\n\u{0060}\u{0060}\u{0060}text\n{text}\u{0060}\u{0060}\u{0060}\n")).map_err(|e|CliError::io(e.to_string()))?;
    // Compare's canonical CI summary is summary.md.
    let summary = dir.join("summary.md");
    if summary.is_file() {
        let existing =
            std::fs::read_to_string(&summary).map_err(|e| CliError::io(e.to_string()))?;
        let existing = existing
            .split("<!-- saccade-review-summary -->")
            .next()
            .unwrap_or("");
        let block = std::fs::read_to_string(md).map_err(|e| CliError::io(e.to_string()))?;
        std::fs::write(
            summary,
            format!("{existing}\n<!-- saccade-review-summary -->\n{block}"),
        )
        .map_err(|e| CliError::io(e.to_string()))?;
    }
    Ok(())
}
pub(crate) fn review(args: ReviewArgs) -> Result<u8, CliError> {
    let (value, text) = review_run(&args, None)?;
    crate::emit(&if args.json {
        format!("{}\n", serde_json::to_string_pretty(&value)?)
    } else {
        text
    })?;
    Ok(0)
}
#[cfg(feature = "evaluation")]
pub(crate) fn bench(args: BenchArgs, read_root: Option<&Path>) -> Result<Value, CliError> {
    let labels = saccade_core::labels::Labels::read(&args.labels)?;
    if let Some(root) = read_root {
        for i in &labels.items {
            for image in &i.images {
                let path = saccade_core::paths::canonicalize(saccade_core::paths::resolve(
                    image,
                    &args.labels,
                ))
                .map_err(|e| CliError::io(e.to_string()))?;
                if !path.starts_with(root) {
                    return Err(CliError::new("unsafe_path", "label image escapes MCP root"));
                }
            }
        }
    }
    let profile = Profile::default();
    let models = if args.models.is_empty() {
        profile.gemini_models.clone()
    } else {
        args.models
    };
    let backend = LiveBackend::new(
        Keys::new(args.keys_dir),
        Retry {
            max_retries: profile.max_retries,
            base_ms: 500,
        },
        Duration::from_secs(60),
    )
    .with_policy(
        args.cache_dir
            .unwrap_or_else(saccade_core::local::default_cache_dir),
        profile.cooldown_secs,
        args.budget_calls,
        args.jev_call_limit,
        args.gemini_call_limit,
    );
    let value = saccade_core::judge_bench::run(
        &args.labels,
        &labels,
        &models,
        &args.questions,
        &backend,
        args.budget_calls,
    )?;
    review::write_json(&args.out, &value)?;
    Ok(value)
}
#[cfg(feature = "evaluation")]
pub(crate) fn collect(args: CollectArgs) -> Result<Value, CliError> {
    let dir = args
        .decisions_dir
        .unwrap_or_else(saccade_core::local::default_decisions_dir);
    let labels = saccade_core::labels::collect(Some(&dir), &args.reports, &args.out)?;
    Ok(serde_json::to_value(labels)?)
}

#[cfg(feature = "mcp")]
mod bindings {
    use super::*;
    pub(crate) fn schemas() -> Vec<Value> {
        vec![
            json!({"name":"saccade_review","description":"Experimental cascade: deterministic gate, batched Jev, Gemini only on escalation, human inbox and blind votes. Never approves on model output. budget_calls counts actual HTTP attempts including retries. Sends evidence to providers.",
            "inputSchema":{"type":"object","properties":{"report_json":{"type":"string"},"profile":{"type":"string"},"intent":{"type":"string"},
                "budget_calls":{"type":"integer","minimum":0,"maximum":1000},"max_gemini":{"type":"integer","minimum":0,"maximum":100},
                "dry_run":{"type":"boolean"}},"required":["report_json"],"additionalProperties":false},
            "outputSchema":{"type":"object","properties":{"schema":{"const":review::SCHEMA},"entries":{"type":"array","items":{"type":"object"}}},"required":["schema","entries"]},
            "annotations":{"readOnlyHint":false,"destructiveHint":false,"openWorldHint":true}}),
            #[cfg(feature = "evaluation")]
            json!({"name":"saccade_judge_bench","description":"Benchmark the same evidence-bound human labels across Jev and every pinned Gemini model, in both orders. Measures accuracy, ECE, latency and position bias; suggests an empirically measured chain. Actual HTTP budget includes retries.",
            "inputSchema":{"type":"object","properties":{"labels":{"type":"string"},"models":{"type":"array","items":{"type":"string"}},
                "questions":{"type":"array","items":{"type":"string"}},"budget_calls":{"type":"integer","minimum":0,"maximum":1000},
                "out":{"type":"string"}},"required":["labels"],"additionalProperties":false},
            "outputSchema":{"type":"object","properties":{"schema":{"const":saccade_core::judge_bench::SCHEMA},"models":{"type":"array","items":{"type":"object"}}},"required":["schema","models"]},
            "annotations":{"readOnlyHint":false,"destructiveHint":false,"openWorldHint":true}}),
        ]
    }
    type Resolve<'a> = &'a dyn Fn(&str, &str) -> Result<PathBuf, CliError>;
    fn string(args: &Map<String, Value>, key: &str) -> Result<Option<String>, CliError> {
        match args.get(key) {
            None => Ok(None),
            Some(Value::String(s)) => Ok(Some(s.clone())),
            _ => Err(CliError::usage(format!("{key} must be a string"))),
        }
    }
    fn number(args: &Map<String, Value>, key: &str, default: usize) -> Result<usize, CliError> {
        match args.get(key) {
            None => Ok(default),
            Some(v) => v
                .as_u64()
                .filter(|n| *n <= 1000)
                .map(|n| n as usize)
                .ok_or_else(|| CliError::usage(format!("{key} must be an integer from 0 to 1000"))),
        }
    }
    #[cfg(feature = "evaluation")]
    fn strings(args: &Map<String, Value>, key: &str) -> Result<Vec<String>, CliError> {
        match args.get(key) {
            None => Ok(Vec::new()),
            Some(Value::Array(a)) => a
                .iter()
                .map(|s| {
                    s.as_str()
                        .map(str::to_owned)
                        .ok_or_else(|| CliError::usage(format!("{key} must contain strings")))
                })
                .collect(),
            _ => Err(CliError::usage(format!("{key} must be an array"))),
        }
    }
    pub(crate) fn call(
        name: &str,
        args: &Map<String, Value>,
        resolve: Resolve<'_>,
    ) -> Option<Result<(Value, String), CliError>> {
        if !["saccade_review", "saccade_judge_bench"].contains(&name) {
            return None;
        }
        Some((|| {
            let known = if name == "saccade_review" {
                vec![
                    "report_json",
                    "profile",
                    "intent",
                    "budget_calls",
                    "max_gemini",
                    "dry_run",
                ]
            } else {
                vec!["labels", "models", "questions", "budget_calls", "out"]
            };
            if let Some(k) = args.keys().find(|k| !known.contains(&k.as_str())) {
                return Err(CliError::usage(format!("unknown argument {k}")));
            }
            let root = root(Some(&resolve("root", ".")?))?;
            if name == "saccade_review" {
                let path = string(args, "report_json")?
                    .ok_or_else(|| CliError::usage("report_json is required"))?;
                let profile = string(args, "profile")?.unwrap_or_else(|| "nightly".into());
                let profile = if Profile::template(&profile).is_some() {
                    profile
                } else {
                    saccade_core::paths::portable(&resolve("profile", &profile)?)
                };
                let dry = match args.get("dry_run") {
                    None => false,
                    Some(Value::Bool(b)) => *b,
                    _ => return Err(CliError::usage("dry_run must be boolean")),
                };
                review_run(
                    &ReviewArgs {
                        report_json: resolve("report_json", &path)?,
                        profile,
                        intent: string(args, "intent")?,
                        intent_file: None,
                        budget_calls: number(args, "budget_calls", 30)?,
                        max_gemini: args
                            .get("max_gemini")
                            .map(|_| number(args, "max_gemini", 5))
                            .transpose()?,
                        json: true,
                        dry_run: dry,
                        keys_dir: None,
                        cache_dir: Some(resolve("cache_dir", ".saccade-review/cache")?),
                        decisions_dir: Some(resolve("decisions_dir", "judge-decisions")?),
                        serve_root: Some(root.clone()),
                        ocr_cmd: None,
                    },
                    Some(&root),
                )
            } else {
                #[cfg(not(feature = "evaluation"))]
                return Err(saccade_core::Error::FeatureUnavailable {
                    feature: "evaluation",
                }
                .into());
                #[cfg(feature = "evaluation")]
                {
                    let labels = string(args, "labels")?
                        .ok_or_else(|| CliError::usage("labels is required"))?;
                    let value = bench(
                        BenchArgs {
                            labels: resolve("labels", &labels)?,
                            models: strings(args, "models")?,
                            questions: strings(args, "questions")?,
                            budget_calls: number(args, "budget_calls", 30)?,
                            keys_dir: None,
                            cache_dir: Some(resolve("cache_dir", ".saccade-review/cache")?),
                            jev_call_limit: 15,
                            gemini_call_limit: 30,
                            out: resolve(
                                "out",
                                &string(args, "out")?
                                    .unwrap_or_else(|| "saccade-judge-bench.v1.json".into()),
                            )?,
                        },
                        Some(&root),
                    )?;
                    let text = format!(
                        "bench: {} items; {} calls; suggested chain: {}",
                        value["items"], value["calls_used"], value["suggested_chain"]
                    );
                    Ok((value, text))
                }
            }
        })())
    }
}
#[cfg(feature = "mcp")]
pub(crate) use bindings::{call, schemas};
