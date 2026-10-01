//! `flipdiff mcp`: a Model Context Protocol server over stdio.
//!
//! Hand-rolled JSON-RPC 2.0, one JSON message per line (the stdio transport of
//! protocol version `2025-06-18`). It implements `initialize`,
//! `notifications/initialized`, `ping`, `tools/list` and `tools/call`.
//!
//! Tool failures are tool results with `isError: true` and a stable code in
//! `structuredContent.code`: `usage` (bad or missing argument, unsafe path),
//! `io` (a path cannot be read or written, an image or report does not decode)
//! or `config` (a config file or setting is invalid). A regression is not an
//! error: the result says `verdict: "regression"`.

use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

use flipdiff_core::config::RunConfig;
use flipdiff_core::explain::{ExplainOptions, explain};
use flipdiff_core::report::{Labels, Metric, Mode, Report};
use serde_json::{Map, Value, json};

use crate::agent::{DEFAULT_TOP_FAILING, summary_text, summary_value};

/// MCP protocol version this server speaks.
pub const PROTOCOL_VERSION: &str = "2025-06-18";

/// A failed tool call.
#[derive(Debug)]
struct ToolError {
    code: &'static str,
    message: String,
}

fn usage(message: impl Into<String>) -> ToolError {
    ToolError {
        code: "usage",
        message: message.into(),
    }
}

fn io_error(message: impl Into<String>) -> ToolError {
    ToolError {
        code: "io",
        message: message.into(),
    }
}

impl From<flipdiff_core::Error> for ToolError {
    fn from(e: flipdiff_core::Error) -> Self {
        let code = match e {
            flipdiff_core::Error::Config(_) => "config",
            _ => "io",
        };
        ToolError {
            code,
            message: e.to_string(),
        }
    }
}

type ToolResult = Result<(Value, String), ToolError>;

fn tool_schemas() -> Value {
    let dir = |what: &str| json!({"type": "string", "minLength": 1, "description": what});
    let report_json = json!({
        "type": "string", "minLength": 1,
        "description": "Path to flipdiff-report.v1.json, resolved against the server's working directory."
    });
    json!([
        {
            "name": "flipdiff_compare",
            "title": "Compare captures against baselines",
            "description": "Perceptual (NVIDIA FLIP) regression check of capture_dir against baseline_dir. Writes the report (index.html, flipdiff-report.v1.json, images) to out_dir, and an explain pack with hotspot crops to out_dir/explain when anything fails. A regression is a normal result (verdict: \"regression\"), not an error. Without `config`, ./flipdiff.toml in the server's working directory is used when present.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "baseline_dir": dir("Directory of approved baseline images."),
                    "capture_dir": dir("Directory of fresh captures."),
                    "out_dir": dir("Report output directory. Must not be inside baseline_dir or capture_dir; its previous report files are replaced."),
                    "threshold": {"type": "number", "description": "Default pass threshold (default 0.01)."},
                    "metric": {"type": "string", "enum": ["mean", "p95", "max"], "description": "Deciding metric (default mean)."},
                    "config": {"type": "string", "description": "flipdiff.toml path (thresholds, regions, masks, hotspots)."},
                    "require_matching_meta": {"type": "boolean", "description": "Make an undeclared metadata-sidecar difference an error."},
                    "declare": {"type": "array", "items": {"type": "string"}, "description": "Sidecar keys or globs allowed to differ; needs require_matching_meta."}
                },
                "required": ["baseline_dir", "capture_dir", "out_dir"],
                "additionalProperties": false
            }
        },
        {
            "name": "flipdiff_identity",
            "title": "Check a candidate build against its parent",
            "description": "Strict sameness check: metric max, threshold 0, bit-identity reported per image. Writes the report to out_dir like flipdiff_compare. No config file is read.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "parent_dir": dir("Directory of images from the parent build."),
                    "candidate_dir": dir("Directory of images from the candidate build."),
                    "out_dir": dir("Report output directory. Must not be inside either input."),
                    "threshold": {"type": "number", "description": "Pass threshold for images that are not bit-identical (default 0)."}
                },
                "required": ["parent_dir", "candidate_dir", "out_dir"],
                "additionalProperties": false
            }
        },
        {
            "name": "flipdiff_explain",
            "title": "Write the hotspot judge pack",
            "description": "From a report JSON, writes per-hotspot strips [baseline | capture | heatmap], a whole-frame strip with the hotspot boxes, explain.json and explain.md to out_dir. With blind, strips are [A | B] in a random order without the heatmap and the key goes to blind-key.json.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "report_json": report_json,
                    "out_dir": dir("Directory for the pack. Only the pack's own files are replaced."),
                    "top": {"type": "integer", "minimum": 1, "maximum": 20, "description": "Hotspots per entry (default 3)."},
                    "blind": {"type": "boolean", "description": "Hide which side is which and omit the heatmap."}
                },
                "required": ["report_json", "out_dir"],
                "additionalProperties": false
            }
        },
        {
            "name": "flipdiff_summary",
            "title": "Summarise a report",
            "description": "Verdict, totals, the worst failing entries with their hotspots and the report's file paths, without re-running anything.",
            "inputSchema": {
                "type": "object",
                "properties": { "report_json": report_json },
                "required": ["report_json"],
                "additionalProperties": false
            }
        }
    ])
}

fn arg_str(args: &Map<String, Value>, key: &str) -> Result<Option<String>, ToolError> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) if !s.is_empty() => Ok(Some(s.clone())),
        Some(_) => Err(usage(format!("`{key}` must be a non-empty string"))),
    }
}

fn require_str(args: &Map<String, Value>, key: &str) -> Result<String, ToolError> {
    arg_str(args, key)?.ok_or_else(|| usage(format!("`{key}` is required")))
}

fn arg_bool(args: &Map<String, Value>, key: &str) -> Result<Option<bool>, ToolError> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Bool(b)) => Ok(Some(*b)),
        Some(_) => Err(usage(format!("`{key}` must be a boolean"))),
    }
}

fn arg_f64(args: &Map<String, Value>, key: &str) -> Result<Option<f64>, ToolError> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v
            .as_f64()
            .filter(|n| n.is_finite())
            .map(Some)
            .ok_or_else(|| usage(format!("`{key}` must be a finite number"))),
    }
}

fn arg_top(args: &Map<String, Value>) -> Result<usize, ToolError> {
    match args.get("top") {
        None | Some(Value::Null) => Ok(3),
        Some(v) => v
            .as_u64()
            .filter(|n| (1..=20).contains(n))
            .map(|n| n as usize)
            .ok_or_else(|| usage("`top` must be an integer from 1 to 20")),
    }
}

fn arg_strings(args: &Map<String, Value>, key: &str) -> Result<Vec<String>, ToolError> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::Array(items)) => items
            .iter()
            .map(|v| {
                v.as_str()
                    .map(str::to_owned)
                    .ok_or_else(|| usage(format!("`{key}` must be an array of strings")))
            })
            .collect(),
        Some(_) => Err(usage(format!("`{key}` must be an array of strings"))),
    }
}

fn reject_unknown(args: &Map<String, Value>, known: &[&str]) -> Result<(), ToolError> {
    match args.keys().find(|k| !known.contains(&k.as_str())) {
        Some(k) => Err(usage(format!("unknown argument `{k}`"))),
        None => Ok(()),
    }
}

/// Resolves an agent-supplied path against the server's working directory.
fn resolve(p: &str) -> PathBuf {
    let path = Path::new(p);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().map_or_else(|_| path.to_path_buf(), |c| c.join(path))
    }
}

fn existing_dir(key: &str, p: &str) -> Result<PathBuf, ToolError> {
    let path = resolve(p);
    if path.is_dir() {
        Ok(path)
    } else {
        Err(io_error(format!(
            "`{key}`: {} is not a directory",
            path.display()
        )))
    }
}

fn existing_file(key: &str, p: &str) -> Result<PathBuf, ToolError> {
    let path = resolve(p);
    if path.is_file() {
        Ok(path)
    } else {
        Err(io_error(format!(
            "`{key}`: {} is not a file",
            path.display()
        )))
    }
}

/// `path` with `..` and `.` folded away and the longest existing prefix
/// canonicalised (symlinks resolved), so a not-yet-created output directory
/// can be compared with an input.
fn normalise(path: &Path) -> PathBuf {
    let mut folded = PathBuf::new();
    for c in path.components() {
        match c {
            std::path::Component::ParentDir => {
                folded.pop();
            }
            std::path::Component::CurDir => {}
            other => folded.push(other.as_os_str()),
        }
    }
    let mut existing = folded.clone();
    let mut tail = Vec::new();
    while !existing.exists() {
        match existing.file_name().map(std::ffi::OsStr::to_owned) {
            Some(name) => tail.push(name),
            None => break,
        }
        if !existing.pop() {
            break;
        }
    }
    let mut out = existing.canonicalize().unwrap_or(existing);
    for name in tail.into_iter().rev() {
        out.push(name);
    }
    out
}

/// Validates `out_dir` against the input directories: it must not be one of
/// them, inside one, or contain an input under its `images/` folder (which a
/// run clears).
fn checked_out_dir(out: &str, inputs: &[&Path]) -> Result<PathBuf, ToolError> {
    let out = resolve(out);
    let norm = normalise(&out);
    for input in inputs {
        let input = normalise(input);
        if norm.starts_with(&input) || input.starts_with(norm.join("images")) {
            return Err(usage(format!(
                "`out_dir` {} must not be inside an input directory ({})",
                out.display(),
                input.display()
            )));
        }
    }
    Ok(out)
}

fn finish_run(report: &Report, report_json: &Path, explain_pack: Option<String>) -> ToolResult {
    let mut value = summary_value(report, report_json, DEFAULT_TOP_FAILING);
    if let (Some(err), Some(obj)) = (explain_pack, value.as_object_mut()) {
        obj.insert("explain_error".into(), Value::String(err));
    }
    let text = summary_text(report, &value);
    Ok((value, text))
}

/// Runs a comparison and, when something fails, writes the explain pack.
fn run_and_explain(
    baseline: &Path,
    capture: &Path,
    out: &Path,
    cfg: &RunConfig,
    explain_failing: bool,
) -> ToolResult {
    let report = flipdiff_core::run::run(baseline, capture, out, cfg)?;
    let report_json = out.join(flipdiff_core::report::REPORT_FILE_NAME);
    let mut explain_error = None;
    if explain_failing && report.totals.fail > 0 {
        let opts = ExplainOptions::default();
        if let Err(e) = explain(&report_json, &out.join("explain"), &opts) {
            explain_error = Some(e.to_string());
        }
    }
    finish_run(&report, &report_json, explain_error)
}

fn tool_compare(args: &Map<String, Value>) -> ToolResult {
    reject_unknown(
        args,
        &[
            "baseline_dir",
            "capture_dir",
            "out_dir",
            "threshold",
            "metric",
            "config",
            "require_matching_meta",
            "declare",
        ],
    )?;
    let baseline = existing_dir("baseline_dir", &require_str(args, "baseline_dir")?)?;
    let capture = existing_dir("capture_dir", &require_str(args, "capture_dir")?)?;
    let out = checked_out_dir(&require_str(args, "out_dir")?, &[&baseline, &capture])?;
    let mut cfg = match arg_str(args, "config")? {
        Some(p) => RunConfig::from_toml_file(&existing_file("config", &p)?)?,
        None => {
            let auto = resolve("flipdiff.toml");
            if auto.is_file() {
                RunConfig::from_toml_file(&auto)?
            } else {
                RunConfig::default()
            }
        }
    };
    if let Some(t) = arg_f64(args, "threshold")? {
        cfg.default_threshold = t;
    }
    if let Some(m) = arg_str(args, "metric")? {
        cfg.default_metric = match m.as_str() {
            "mean" => Metric::Mean,
            "p95" => Metric::P95,
            "max" => Metric::Max,
            other => {
                return Err(usage(format!(
                    "`metric` must be mean, p95 or max, got {other:?}"
                )));
            }
        };
    }
    let required = arg_bool(args, "require_matching_meta")?.unwrap_or(false);
    let declared = arg_strings(args, "declare")?;
    if !declared.is_empty() && !required {
        return Err(usage("`declare` needs `require_matching_meta: true`"));
    }
    cfg.meta.required |= required;
    cfg.meta.declared.extend(declared);
    cfg.validate()?;
    run_and_explain(&baseline, &capture, &out, &cfg, true)
}

fn tool_identity(args: &Map<String, Value>) -> ToolResult {
    reject_unknown(
        args,
        &["parent_dir", "candidate_dir", "out_dir", "threshold"],
    )?;
    let parent = existing_dir("parent_dir", &require_str(args, "parent_dir")?)?;
    let candidate = existing_dir("candidate_dir", &require_str(args, "candidate_dir")?)?;
    let out = checked_out_dir(&require_str(args, "out_dir")?, &[&parent, &candidate])?;
    let cfg = RunConfig {
        mode: Mode::Identity,
        labels: Labels {
            baseline: "parent".into(),
            capture: "candidate".into(),
        },
        default_threshold: arg_f64(args, "threshold")?.unwrap_or(0.0),
        default_metric: Metric::Max,
        ..RunConfig::default()
    };
    cfg.validate()?;
    run_and_explain(&parent, &candidate, &out, &cfg, true)
}

fn tool_explain(args: &Map<String, Value>) -> ToolResult {
    reject_unknown(args, &["report_json", "out_dir", "top", "blind"])?;
    let report_json = existing_file("report_json", &require_str(args, "report_json")?)?;
    let out = resolve(&require_str(args, "out_dir")?);
    let blind = arg_bool(args, "blind")?.unwrap_or(false);
    let opts = ExplainOptions {
        top: arg_top(args)?,
        blind,
        ..ExplainOptions::default()
    };
    let pack = explain(&report_json, &out, &opts)?;
    let abs = |rel: &str| out.join(rel).display().to_string();
    let entries: Vec<Value> = pack
        .entries
        .iter()
        .map(|e| {
            json!({
                "name": e.name,
                "status": e.status,
                "value": e.value,
                "thumbnail": e.thumbnail.as_deref().map(abs),
                "note": e.note,
                "hotspots": e.hotspots.iter().map(|h| json!({
                    "index": h.index,
                    "strip": abs(&h.strip),
                    "position": h.hotspot.position,
                    "rect_px": h.hotspot.rect_px,
                    "share_of_total_error": h.hotspot.share_of_total_error,
                    "panels": h.panels,
                })).collect::<Vec<_>>(),
            })
        })
        .collect();
    let strips: usize = pack.entries.iter().map(|e| e.hotspots.len()).sum();
    let value = json!({
        "schema": "flipdiff-explain-result.v1",
        "blind": blind,
        "paths": {
            "dir": out.display().to_string(),
            "explain_json": abs(flipdiff_core::explain::EXPLAIN_FILE),
            "explain_md": abs(flipdiff_core::explain::EXPLAIN_MD_FILE),
            "blind_key": blind.then(|| abs(flipdiff_core::explain::EXPLAIN_BLIND_KEY_FILE)),
        },
        "entries": entries,
    });
    let text = format!(
        "flipdiff explain: {} entries, {strips} hotspot strips{}\nsummary: {}",
        pack.entries.len(),
        if blind {
            " (blind; key in blind-key.json, keep it from the judge)"
        } else {
            ""
        },
        abs(flipdiff_core::explain::EXPLAIN_MD_FILE)
    );
    Ok((value, text))
}

fn tool_summary(args: &Map<String, Value>) -> ToolResult {
    reject_unknown(args, &["report_json"])?;
    let path = existing_file("report_json", &require_str(args, "report_json")?)?;
    let text = std::fs::read_to_string(&path)
        .map_err(|e| io_error(format!("reading {}: {e}", path.display())))?;
    let report: Report = serde_json::from_str(&text)
        .map_err(|e| io_error(format!("parsing {}: {e}", path.display())))?;
    let value = summary_value(&report, &path, DEFAULT_TOP_FAILING);
    let text = summary_text(&report, &value);
    Ok((value, text))
}

fn call_tool(name: &str, args: &Map<String, Value>) -> Option<ToolResult> {
    Some(match name {
        "flipdiff_compare" => tool_compare(args),
        "flipdiff_identity" => tool_identity(args),
        "flipdiff_explain" => tool_explain(args),
        "flipdiff_summary" => tool_summary(args),
        _ => return None,
    })
}

fn tool_response(result: ToolResult) -> Value {
    match result {
        Ok((structured, text)) => json!({
            "content": [{"type": "text", "text": text}],
            "structuredContent": structured,
            "isError": false,
        }),
        Err(e) => json!({
            "content": [{"type": "text", "text": format!("flipdiff error [{}]: {}", e.code, e.message)}],
            "structuredContent": {"schema": "flipdiff-error.v1", "code": e.code, "message": e.message},
            "isError": true,
        }),
    }
}

fn rpc_error(id: Value, code: i64, message: &str) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
}

/// Handles one JSON-RPC message; `None` for notifications (no reply).
pub fn handle_message(msg: &Value) -> Option<Value> {
    let Some(obj) = msg.as_object() else {
        return Some(rpc_error(
            Value::Null,
            -32600,
            "invalid request: expected a JSON object",
        ));
    };
    let id = obj.get("id").cloned();
    let Some(method) = obj.get("method").and_then(Value::as_str) else {
        // A response or garbage from the client: nothing to answer.
        return id.map(|id| rpc_error(id, -32600, "invalid request: missing method"));
    };
    // No id: a notification such as notifications/initialized, never answered.
    let id = id?;
    let params = obj.get("params").and_then(Value::as_object);
    let result = match method {
        "initialize" => json!({
            "protocolVersion": PROTOCOL_VERSION,
            "capabilities": {"tools": {"listChanged": false}},
            "serverInfo": {"name": "flipdiff", "version": env!("CARGO_PKG_VERSION")},
            "instructions": "Perceptual (FLIP) image regression. flipdiff_compare or flipdiff_identity writes a report plus hotspot crops; read structuredContent.failing[].hotspots for where the visible difference is, and the explain pack strips to see it. Paths are relative to the server's working directory.",
        }),
        "ping" => json!({}),
        "tools/list" => json!({"tools": tool_schemas()}),
        "tools/call" => {
            let Some(name) = params.and_then(|p| p.get("name")).and_then(Value::as_str) else {
                return Some(rpc_error(id, -32602, "tools/call needs a string `name`"));
            };
            let empty = Map::new();
            let args = match params.and_then(|p| p.get("arguments")) {
                None | Some(Value::Null) => &empty,
                Some(Value::Object(a)) => a,
                Some(_) => return Some(rpc_error(id, -32602, "`arguments` must be an object")),
            };
            match call_tool(name, args) {
                Some(r) => tool_response(r),
                None => return Some(rpc_error(id, -32602, &format!("unknown tool {name:?}"))),
            }
        }
        other => return Some(rpc_error(id, -32601, &format!("method not found: {other}"))),
    };
    Some(json!({"jsonrpc": "2.0", "id": id, "result": result}))
}

/// Serves MCP on stdin/stdout until stdin closes.
pub fn serve_stdio() -> Result<(), String> {
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    for line in stdin.lock().lines() {
        let line = line.map_err(|e| format!("reading stdin: {e}"))?;
        if line.trim().is_empty() {
            continue;
        }
        let reply = match serde_json::from_str::<Value>(&line) {
            Ok(msg) => handle_message(&msg),
            Err(e) => Some(rpc_error(Value::Null, -32700, &format!("parse error: {e}"))),
        };
        if let Some(reply) = reply {
            let mut out = stdout.lock();
            let text = serde_json::to_string(&reply).map_err(|e| e.to_string())?;
            if out
                .write_all(text.as_bytes())
                .and_then(|()| out.write_all(b"\n"))
                .and_then(|()| out.flush())
                .is_err()
            {
                return Ok(()); // client went away
            }
        }
    }
    Ok(())
}
