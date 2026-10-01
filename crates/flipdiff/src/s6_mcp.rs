//! MCP bindings for S6. Shell capture is deliberately absent.
use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use flipdiff_core::inbox::Question;
use flipdiff_core::watch::{WatchHandle, WatchOptions};
use serde_json::{Map, Value, json};

use crate::agent::{CliError, DEFAULT_TOP_FAILING, result_value};

#[derive(Default)]
pub(crate) struct Watches {
    latest: Arc<Mutex<HashMap<PathBuf, Value>>>,
    handles: Vec<WatchHandle>,
    initialized: Arc<AtomicBool>,
}
impl Watches {
    pub fn activate(&self) {
        self.initialized.store(true, Ordering::SeqCst);
    }
    pub fn start(
        &mut self,
        specs: &[String],
        resolve: &impl Fn(&str, &str) -> Result<PathBuf, CliError>,
    ) -> Result<(), CliError> {
        for (i, spec) in specs.iter().enumerate() {
            let (base, cap) = spec
                .split_once(':')
                .filter(|(a, b)| !a.is_empty() && !b.is_empty())
                .ok_or_else(|| CliError::usage("--watch needs baseline:capture"))?;
            let baseline = resolve("baseline", base)?;
            let capture = resolve("capture", cap)?;
            let out = resolve("out", &format!(".flipdiff-watch/{i}"))?;
            let config = resolve("config", "flipdiff.toml")?;
            let config = if config.is_file() {
                flipdiff_core::config::RunConfig::from_toml_file(&config)?
            } else {
                Default::default()
            };
            let opts = WatchOptions {
                baseline,
                capture: capture.clone(),
                out: out.clone(),
                config,
                debounce: Duration::from_millis(500),
            };
            opts.config.validate()?;
            if !opts.baseline.is_dir() || !opts.capture.is_dir() {
                return Err(CliError::usage("watch inputs must be directories"));
            }
            flipdiff_core::run::guard_output_dir(
                &out,
                &[&opts.baseline, &capture],
                &[
                    flipdiff_core::report::REPORT_FILE_NAME,
                    flipdiff_core::run::RUN_SENTINEL,
                ],
            )?;
            if self
                .latest
                .lock()
                .map_err(|_| CliError::io("watch lock unavailable"))?
                .contains_key(&capture)
            {
                return Err(CliError::usage("capture directory watched twice"));
            }
            self.latest
                .lock()
                .map_err(|_| CliError::io("watch lock unavailable"))?
                .insert(
                    capture.clone(),
                    CliError::usage("watch initializing").value(),
                );
            let latest = self.latest.clone();
            let initialized = self.initialized.clone();
            self.handles.push(flipdiff_core::watch::start(opts, move |result| {
                let (value, text) = match result {
                    Ok(report) => (result_value(&report, &out.join(flipdiff_core::report::REPORT_FILE_NAME), DEFAULT_TOP_FAILING, false), crate::s6::watch_text(&report)),
                    Err(e) => { let e: CliError = e.into(); (e.value(), e.to_string()) }
                };
                if let Ok(mut map) = latest.lock() { map.insert(capture.clone(), value.clone()); }
                if initialized.load(Ordering::SeqCst) {
                    let message = json!({"jsonrpc":"2.0","method":"notifications/message","params":{"level":"info","logger":"flipdiff.watch","data":{"capture_dir":capture,"description":text,"result":value}}});
                    if let Ok(line) = serde_json::to_string(&message) { let stdout = std::io::stdout(); let mut stdout = stdout.lock(); let _ = writeln!(stdout, "{line}"); let _ = stdout.flush(); }
                }
            }));
        }
        Ok(())
    }
}

fn str_arg(args: &Map<String, Value>, key: &str) -> Result<Option<String>, CliError> {
    match args.get(key) {
        None => Ok(None),
        Some(Value::String(s)) => Ok(Some(s.clone())),
        _ => Err(CliError::usage(format!("{key} must be a string"))),
    }
}
fn required(args: &Map<String, Value>, key: &str) -> Result<String, CliError> {
    str_arg(args, key)?.ok_or_else(|| CliError::usage(format!("missing {key}")))
}
fn bool_arg(args: &Map<String, Value>, key: &str) -> Result<bool, CliError> {
    match args.get(key) {
        None => Ok(false),
        Some(Value::Bool(b)) => Ok(*b),
        _ => Err(CliError::usage(format!("{key} must be boolean"))),
    }
}
fn strings(args: &Map<String, Value>, key: &str) -> Result<Vec<String>, CliError> {
    let value = args
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| CliError::usage(format!("{key} must be an array of strings")))?;
    value
        .iter()
        .map(|v| {
            v.as_str()
                .map(str::to_owned)
                .ok_or_else(|| CliError::usage(format!("{key} must contain strings")))
        })
        .collect()
}
fn unknown(args: &Map<String, Value>, keys: &[&str]) -> Result<(), CliError> {
    if let Some(key) = args.keys().find(|k| !keys.contains(&k.as_str())) {
        return Err(CliError::usage(format!("unknown argument {key}")));
    }
    Ok(())
}
fn cache(
    args: &Map<String, Value>,
    resolve: &impl Fn(&str, &str) -> Result<PathBuf, CliError>,
) -> Result<PathBuf, CliError> {
    str_arg(args, "cache_dir")?.map_or_else(
        || Ok(flipdiff_core::serve::default_cache_dir()),
        |p| resolve("cache_dir", &p),
    )
}

pub(crate) fn call(
    name: &str,
    args: &Map<String, Value>,
    watches: &Watches,
    resolve: &impl Fn(&str, &str) -> Result<PathBuf, CliError>,
) -> Option<Result<(Value, String), CliError>> {
    match name {
        "flipdiff_bisect" => Some((|| {
            unknown(
                args,
                &["runs", "good", "out_dir", "threshold", "metric", "entries"],
            )?;
            let runs = strings(args, "runs")?
                .iter()
                .map(|p| resolve("runs", p))
                .collect::<Result<Vec<_>, _>>()?;
            let good = str_arg(args, "good")?
                .map(|p| resolve("good", &p))
                .transpose()?;
            let out = resolve("out_dir", &required(args, "out_dir")?)?;
            let threshold = match args.get("threshold") {
                None => None,
                Some(v) => Some(
                    v.as_f64()
                        .ok_or_else(|| CliError::usage("threshold must be a number"))?,
                ),
            };
            let opts = crate::s6::options(
                threshold,
                str_arg(args, "metric")?.as_deref(),
                str_arg(args, "entries")?,
            )?;
            let result = flipdiff_core::bisect::runs(&runs, good.as_deref(), &out, &opts)?;
            Ok((serde_json::to_value(&result)?, result.text()))
        })()),
        "flipdiff_watch_status" => Some((|| {
            unknown(args, &["capture_dir"])?;
            let path = resolve("capture_dir", &required(args, "capture_dir")?)?;
            let value = watches
                .latest
                .lock()
                .map_err(|_| CliError::io("watch lock unavailable"))?
                .get(&path)
                .cloned()
                .ok_or_else(|| {
                    CliError::usage("directory is not watched; start mcp --watch BASE:CAP")
                })?;
            Ok((value, format!("latest watch result for {}", path.display())))
        })()),
        "flipdiff_ask_human" => Some((|| {
            unknown(
                args,
                &[
                    "serve",
                    "cache_dir",
                    "question",
                    "allowed_answers",
                    "context",
                    "link",
                    "from",
                    "wait",
                    "timeout",
                ],
            )?;
            let client =
                crate::s6::InboxClient::new(&required(args, "serve")?, &cache(args, resolve)?)?;
            let question = Question {
                question: required(args, "question")?,
                allowed_answers: strings(args, "allowed_answers")?,
                context: str_arg(args, "context")?,
                link: str_arg(args, "link")?,
                from: str_arg(args, "from")?,
            };
            let timeout = match args.get("timeout") {
                None => 600,
                Some(v) => v
                    .as_u64()
                    .ok_or_else(|| CliError::usage("timeout must be an unsigned integer"))?,
            };
            let result = client.ask(question, bool_arg(args, "wait")?, timeout)?;
            Ok((
                serde_json::to_value(&result)?,
                format!("{}: {}; {}", result.id, result.status, result.url),
            ))
        })()),
        "flipdiff_inbox_get" => Some((|| {
            unknown(args, &["serve", "cache_dir", "id"])?;
            let client =
                crate::s6::InboxClient::new(&required(args, "serve")?, &cache(args, resolve)?)?;
            let item = client.get(&required(args, "id")?)?;
            Ok((
                serde_json::to_value(&item)?,
                format!("{}: {}", item.id, item.status),
            ))
        })()),
        _ => None,
    }
}

fn output(name: &str) -> Value {
    let source = match name {
        "bisect" => include_str!("../../../schemas/flipdiff-bisect.v1.schema.json"),
        "inbox" => include_str!("../../../schemas/flipdiff-inbox-item.v1.schema.json"),
        "ask" => include_str!("../../../schemas/flipdiff-ask-result.v1.schema.json"),
        _ => include_str!("../../../schemas/flipdiff-result.v1.schema.json"),
    };
    serde_json::from_str(source).unwrap_or_else(|_| json!({"type":"object"}))
}

pub(crate) fn schemas() -> Vec<Value> {
    let string = json!({"type":"string","minLength":1});
    let array = json!({"type":"array","items":{"type":"string","minLength":1},"minItems":1});
    let tool = |name, description, properties, required, output, read_only| json!({"name":name,"description":description,"inputSchema":{"type":"object","properties":properties,"required":required,"additionalProperties":false},"outputSchema":output,"annotations":{"readOnlyHint":read_only,"destructiveHint":false,"idempotentHint":read_only,"openWorldHint":false}});
    vec![
        tool(
            "flipdiff_bisect",
            "Find the first diverging existing run (ordered oldest to newest). Identity by default. Capture-command mode is CLI-only because it executes user shell code.",
            json!({"runs":array,"good":string,"out_dir":string,"threshold":{"type":"number","minimum":0,"maximum":1},"metric":{"enum":["mean","p95","p99","max"]},"entries":string}),
            json!(["runs", "out_dir"]),
            output("bisect"),
            false,
        ),
        tool(
            "flipdiff_watch_status",
            "Latest flipdiff-result.v1 for a capture directory started with mcp --watch BASE:CAP. Initialization/setup errors use flipdiff-error.v1.",
            json!({"capture_dir":string}),
            json!(["capture_dir"]),
            json!({"type":"object","anyOf":[output("result"),serde_json::from_str::<Value>(include_str!("../../../schemas/flipdiff-error.v1.schema.json")).unwrap_or_else(|_| json!({"type":"object"}))]}),
            true,
        ),
        tool(
            "flipdiff_ask_human",
            "Post a closed-answer question to local serve; optionally wait up to timeout seconds (default 600). Never approves or changes a baseline. Requires that serve's private discovery cache.",
            json!({"serve":string,"cache_dir":string,"question":string,"allowed_answers":array,"context":string,"link":string,"from":string,"wait":{"type":"boolean"},"timeout":{"type":"integer","minimum":0}}),
            json!(["serve", "question", "allowed_answers"]),
            output("ask"),
            false,
        ),
        tool(
            "flipdiff_inbox_get",
            "Read a local human inbox item and its answer.",
            json!({"serve":string,"cache_dir":string,"id":string}),
            json!(["serve", "id"]),
            output("inbox"),
            true,
        ),
    ]
}
