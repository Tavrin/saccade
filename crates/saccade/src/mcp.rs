//! `saccade mcp [--root DIR]`: a Model Context Protocol server over stdio.
//!
//! Hand-rolled JSON-RPC 2.0, one JSON message per line (the stdio transport of
//! protocol version `2025-06-18`). It implements `initialize`,
//! `notifications/initialized`, `ping`, `tools/list` and `tools/call`.
//!
//! **Path policy.** Every path an agent passes (inputs, outputs, report and
//! config files, the blind key) is resolved against the root (`--root`, default
//! the working directory), canonicalised, and refused with `unsafe_path` unless
//! it lies under the root. `..` and symlinks cannot escape it.
//!
//! Tool failures are tool results with `isError: true` and a stable code in
//! `structuredContent.code`: `usage` (bad or missing argument), `unsafe_path`
//! (outside the root), `io` (a path cannot be read or written, an image or
//! report does not decode) or `config` (a config file or setting is invalid). A
//! regression is not an error: the result says `verdict: "regression"`.
//!
//! Run and explain results carry up to three image content blocks (the top
//! hotspot strips, at most 1024 px wide) unless `include_images` is false.

use std::io::{BufRead, Cursor, Write};
use std::path::{Path, PathBuf};

use saccade_core::config::RunConfig;
use saccade_core::explain::{ExplainOptions, ExplainPack, absolute, explain};
use saccade_core::report::{Labels, Metric, Mode, Report};
use serde_json::{Map, Value, json};

use crate::agent::{
    CliError, DEFAULT_TOP_FAILING, nothing_compared, result_value, round_floats, summary_text,
    summary_value,
};

/// MCP protocol version this server speaks.
pub const PROTOCOL_VERSION: &str = "2025-06-18";

/// Most image content blocks one result carries.
const MAX_IMAGES: usize = 3;

/// Widest image content block, in pixels.
const MAX_IMAGE_WIDTH: u32 = 1024;

/// What a successful tool call returns.
struct ToolOutput {
    structured: Value,
    text: String,
    images: Vec<Value>,
}

type ToolResult = Result<ToolOutput, CliError>;

/// The server: the root every path must stay under.
pub struct Server {
    root: PathBuf,
    watches: crate::s6_mcp::Watches,
}

fn tool_schemas() -> Value {
    let dir = |what: &str| json!({"type": "string", "minLength": 1, "description": what});
    let report_json = json!({
        "type": "string", "minLength": 1,
        "description": "Path to saccade-report.v1.json (the `paths.report_json` of a compare/identity result). Relative paths resolve against the server root."
    });
    let include_images = json!({
        "type": "boolean",
        "description": "Attach up to 3 image content blocks: the top hotspot strips [baseline | capture | heatmap], each at most 1024 px wide. Default true; set false to save tokens."
    });
    let run_props = |a: &str, b: &str| {
        json!({
            "out_dir": dir("Report output directory (created if missing, inside the server root). A previous report in it is replaced; it must not be inside either input directory."),
            "threshold": {"type": "number", "minimum": 0, "description": "Pass threshold in FLIP units (0 = identical, 1 = maximal error). Default 0.01 for compare, 0 for identity."},
            "metric": {"type": "string", "enum": ["mean", "p95", "p99", "max"], "description": "Deciding metric: mean (default for compare; whole-image drift), p95 or p99 (large or local areas) or max (any single bad pixel; identity default)."},
            "ppd": {"type": "number", "exclusiveMinimum": 0, "description": "FLIP pixels per degree of visual angle (default 67, a 0.7 m viewing distance on a 4K monitor)."},
            "labels": {"type": "array", "items": {"type": "string", "minLength": 1}, "minItems": 2, "maxItems": 2, "description": format!("Display names of the two sides, [\"{a}\", \"{b}\"].")},
            "meta_name": {"type": "string", "minLength": 1, "description": "Metadata sidecar file name (default saccade-meta.json); `<stem>.<name>` next to an image overrides the directory-level one."},
            "require_matching_meta": {"type": "boolean", "description": "Make an undeclared metadata-sidecar difference an error for that image."},
            "declare": {"type": "array", "items": {"type": "string"}, "description": "Sidecar keys or globs allowed to differ; needs require_matching_meta."},
            "fail_on_new": {"type": "boolean", "description": "Count an image with no baseline as a regression."},
            "allow_empty": {"type": "boolean", "description": "Accept a run that compared no image pair (otherwise that is a regression)."},
            "include_images": include_images,
        })
    };
    let mut compare_props = run_props("baseline", "capture");
    let mut identity_props = run_props("parent", "candidate");
    if let (Some(c), Some(i)) = (
        compare_props.as_object_mut(),
        identity_props.as_object_mut(),
    ) {
        c.insert(
            "baseline_dir".into(),
            dir("Directory of approved baseline images (inside the server root)."),
        );
        c.insert(
            "capture_dir".into(),
            dir("Directory of fresh captures, paired with the baselines by relative path."),
        );
        c.insert("config".into(), json!({"type": "string", "description": "saccade.toml path (thresholds, regions, masks, hotspots). Default: ./saccade.toml under the root when present."}));
        i.insert(
            "parent_dir".into(),
            dir("Directory of images from the parent build (inside the server root)."),
        );
        i.insert(
            "candidate_dir".into(),
            dir("Directory of images from the candidate build, paired by relative path."),
        );
    }
    let mut sequence_props = compare_props.clone();
    let mut rank_props = run_props("reference", "candidate");
    if let (Some(s), Some(r)) = (sequence_props.as_object_mut(), rank_props.as_object_mut()) {
        s.remove("include_images");
        s.insert("pattern".into(), json!({"type": "string", "description": "Relative-name glob (default *); frames are sorted by trailing integer and paired by index."}));
        r.remove("include_images");
        r.insert(
            "reference_dir".into(),
            dir("Common reference directory inside the server root."),
        );
        r.insert(
            "candidate_dirs".into(),
            json!({"type": "array", "minItems": 1, "items": {"type": "string", "minLength": 1}}),
        );
        r.insert("labels".into(), json!({"type": "array", "items": {"type": "string", "minLength": 1}, "description": "One unique safe directory label per candidate; default directory names."}));
        r.insert(
            "config".into(),
            dir("Optional saccade.toml inside the root; defaults to the root's saccade.toml."),
        );
        for props in [s, r] {
            props.insert(
                "hdr_tonemapper".into(),
                json!({"type": "string", "enum": ["aces", "hable", "reinhard"]}),
            );
            props.insert(
                "hdr_exposures".into(),
                json!({"type": "string", "description": "HDR exposure range START:STOP:N."}),
            );
        }
    }
    json!([
        {
            "name": "saccade_sequence",
            "title": "Compare numbered frame sequences",
            "description": "Pair colour frames by numeric sorted index, measure per-frame FLIP and added temporal instability (capture consecutive-frame mean minus baseline). Writes saccade-sequence.v1.json and a normal per-frame HTML report with a server-rendered SVG curve. Returns a lean saccade-sequence.v1; full frame details stay on disk. Same exit/verdict rules as compare; temporal decode errors are regressions.",
            "inputSchema": {"type": "object", "properties": sequence_props, "required": ["baseline_dir", "capture_dir", "out_dir"], "additionalProperties": false},
            "outputSchema": sequence_output_schema(),
            "annotations": {"readOnlyHint": false, "destructiveHint": false, "idempotentHint": true, "openWorldHint": false}
        },
        {
            "name": "saccade_rank",
            "title": "Rank candidate image directories",
            "description": "Compare each candidate directory against one reference using mean, p95, p99 or max FLIP. Writes rankings, Markdown tables and one normal report per candidate under out_dir/label. Returns lean saccade-rank.v1 with overall competition ranks, mean ranks/metrics, bit-identical counts and report paths. Missing/error comparisons cannot win an overall ranking.",
            "inputSchema": {"type": "object", "properties": rank_props, "required": ["reference_dir", "candidate_dirs", "out_dir"], "additionalProperties": false},
            "outputSchema": rank_output_schema(),
            "annotations": {"readOnlyHint": false, "destructiveHint": false, "idempotentHint": true, "openWorldHint": false}
        },
        {
            "name": "saccade_compare",
            "title": "Compare captures against baselines",
            "description": "Start here for a visual-regression check. Compares every image in capture_dir with the same-named image in baseline_dir using NVIDIA FLIP (a perceptual metric: invisible differences score ~0, visible ones score high), writes the report (index.html, saccade-report.v1.json, heatmaps) to out_dir and, when anything fails, an explain pack with hotspot crops to out_dir/explain. A regression is a normal result (verdict \"regression\"), not an error. Read structuredContent.failing[] (value vs threshold, hotspots = where the error is concentrated, config_differs = capture-setup keys that changed), then the attached images, then follow structuredContent.next_step. Re-running with the same arguments is safe.",
            "inputSchema": {
                "type": "object",
                "properties": compare_props,
                "required": ["baseline_dir", "capture_dir", "out_dir"],
                "additionalProperties": false
            },
            "outputSchema": result_output_schema(),
            "annotations": {"title": "Compare captures against baselines", "readOnlyHint": false, "destructiveHint": false, "idempotentHint": true, "openWorldHint": false}
        },
        {
            "name": "saccade_identity",
            "title": "Check a candidate build against its parent",
            "description": "Use after a change that should not alter any pixel (a refactor, an optimisation): strict defaults (metric max, threshold 0), and bit-identity is reported per image. Same output and next steps as saccade_compare. No config file is read.",
            "inputSchema": {
                "type": "object",
                "properties": identity_props,
                "required": ["parent_dir", "candidate_dir", "out_dir"],
                "additionalProperties": false
            },
            "outputSchema": result_output_schema(),
            "annotations": {"title": "Check a candidate build against its parent", "readOnlyHint": false, "destructiveHint": false, "idempotentHint": true, "openWorldHint": false}
        },
        {
            "name": "saccade_explain",
            "title": "Write the hotspot judge pack",
            "description": "Cut per-hotspot strips [baseline | capture | heatmap], a whole-frame strip with the hotspot boxes, explain.json and explain.md from a report JSON, and attach the top strips as images. saccade_compare already does this for failing runs; call this to change `top`, or for a blind pairwise judgement: with blind, strips are [A | B] in a random order without the heatmap, the pack names neither the report nor the sides, and the key goes to key_out (required, outside out_dir, inside the server root; keep it from whoever judges).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "report_json": report_json,
                    "out_dir": dir("Directory for the pack (inside the server root). Only the pack's own files are replaced."),
                    "top": {"type": "integer", "minimum": 1, "maximum": 20, "description": "Hotspots per entry (default 3)."},
                    "hotspot_min_share": {"type": "number", "minimum": 0, "maximum": 1, "description": "Leave out hotspots carrying less than this share of the total error (default 0.01)."},
                    "blind": {"type": "boolean", "description": "Hide which side is which and omit the heatmap; needs key_out."},
                    "key_out": dir("Blind key file to write, outside out_dir and inside the server root. Required with blind."),
                    "include_images": include_images
                },
                "required": ["report_json", "out_dir"],
                "additionalProperties": false
            },
            "outputSchema": explain_output_schema(),
            "annotations": {"title": "Write the hotspot judge pack", "readOnlyHint": false, "destructiveHint": false, "idempotentHint": true, "openWorldHint": false}
        },
        {
            "name": "saccade_summary",
            "title": "Summarise a report",
            "description": "Read-only: verdict, totals, the worst failing entries with their hotspots and the report's file paths, from an existing report JSON, without re-running anything.",
            "inputSchema": {
                "type": "object",
                "properties": { "report_json": report_json },
                "required": ["report_json"],
                "additionalProperties": false
            },
            "outputSchema": summary_output_schema(),
            "annotations": {"title": "Summarise a report", "readOnlyHint": true, "destructiveHint": false, "idempotentHint": true, "openWorldHint": false}
        },
        {
            "name": "saccade_compare_runs",
            "title": "Compare whole runs against a reference run",
            "description": "Read-only overview for ablations and A/B runs: compares every run in run_dirs (1 to 5 directories of images) with ref_dir, image by image, and returns the matrix summary (saccade-runs.v1). Per run: counts of identical (bit-identical), changed (worst image and its mean FLIP), only-in-ref and only-in-run images, the sidecar keys that differ at run level, and `no_visible_effect: true` when every image is bit-identical (the run changed nothing). `images[]` is the matrix: one row per image name, one cell per run (status, FLIP metrics). Few shared file names set `mismatch`; retry with pair_by_position. Nothing is written.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "ref_dir": dir("The reference run: a directory of images (inside the server root)."),
                    "run_dirs": {"type": "array", "items": {"type": "string", "minLength": 1}, "minItems": 1, "maxItems": 5, "description": "Runs to compare against the reference, paired by relative image path (inside the server root)."},
                    "labels": {"type": "array", "items": {"type": "string", "minLength": 1}, "description": "One label per directory, the reference first (default: directory names)."},
                    "pair_by_position": {"type": "boolean", "description": "Pair each run's images with the reference's by sorted position instead of by name."},
                    "ppd": {"type": "number", "exclusiveMinimum": 0, "description": "FLIP pixels per degree of visual angle (default 67)."},
                    "meta_name": {"type": "string", "minLength": 1, "description": "Run-level sidecar file name (default saccade-meta.json); its keys that differ from the reference's are listed per run."}
                },
                "required": ["ref_dir", "run_dirs"],
                "additionalProperties": false
            },
            "outputSchema": runs_output_schema(),
            "annotations": {"title": "Compare whole runs against a reference run", "readOnlyHint": true, "destructiveHint": false, "idempotentHint": true, "openWorldHint": false}
        }
    ])
}

fn runs_output_schema() -> Value {
    json!({"type": "object",
        "description": "saccade-runs.v1 (schemas/saccade-runs.v1.schema.json); with isError, saccade-error.v1.",
        "properties": {
            "schema": {"const": "saccade-runs.v1"},
            "ref": {"type": "object", "properties": {"label": {"type": "string"}, "path": {"type": "string"}, "images": {"type": "array", "items": {"type": "object"}}}},
            "runs": {"type": "array", "items": {"type": "object", "properties": {
                "label": {"type": "string"},
                "identical": {"type": "integer"}, "changed": {"type": "integer"}, "errors": {"type": "integer"},
                "only_in_ref": {"type": "integer"}, "only_in_run": {"type": "integer"},
                "mismatch": {"type": "boolean"}, "no_visible_effect": {"type": "boolean"},
                "worst": {"type": "object", "properties": {"name": {"type": "string"}, "mean": {"type": "number"}}},
                "config_differs": {"type": "array", "items": {"type": "string"}},
                "summary": {"type": "string"}
            }, "required": ["label", "identical", "changed", "only_in_ref", "only_in_run", "no_visible_effect", "config_differs", "summary"]}},
            "config_differences": {"type": "array", "items": {"type": "object"}},
            "images": {"type": "array", "items": {"type": "object"}},
            "progress": {"type": "object"}
        },
        "required": ["schema", "ref", "runs", "config_differences", "images", "progress"]})
}

fn sequence_output_schema() -> Value {
    json!({"type": "object", "properties": {
        "schema": {"const": "saccade-sequence.v1"},
        "verdict": {"enum": ["pass", "regression"]},
        "totals": totals_schema(),
        "baseline_frames": {"type": "integer"}, "capture_frames": {"type": "integer"},
        "baseline_temporal_mean": {"type": ["number", "null"]},
        "capture_temporal_mean": {"type": ["number", "null"]},
        "temporal_instability": {"type": ["number", "null"]},
        "frames_over_threshold": {"type": "integer"},
        "worst_frame": {"type": ["object", "null"]},
        "temporal_errors": {"type": "array", "items": {"type": "string"}},
        "report_json": {"type": "string"}, "frames_report_json": {"type": "string"}, "index_html": {"type": "string"}
    }, "required": ["schema", "verdict", "totals", "temporal_instability", "frames_over_threshold", "report_json", "index_html"]})
}

fn rank_output_schema() -> Value {
    json!({"type": "object", "properties": {
        "schema": {"const": "saccade-rank.v1"}, "verdict": {"enum": ["pass", "regression"]},
        "metric": {"enum": ["mean", "p95", "p99", "max"]},
        "overall": {"type": "array", "items": {"type": "object", "properties": {
            "label": {"type": "string"}, "rank": {"type": ["integer", "null"]},
            "mean_rank": {"type": ["number", "null"]}, "mean_metric": {"type": ["number", "null"]},
            "bit_identical": {"type": "integer"}, "report_json": {"type": "string"}, "report_html": {"type": "string"}
        }, "required": ["label", "rank", "mean_rank", "mean_metric", "bit_identical", "report_json", "report_html"]}},
        "reference_images": {"type": "integer"}, "common_images": {"type": "integer"},
        "report_json": {"type": "string"}, "index_html": {"type": "string"}, "markdown": {"type": "string"}
    }, "required": ["schema", "verdict", "metric", "overall", "report_json", "index_html", "markdown"]})
}

fn totals_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "total": {"type": "integer"}, "pass": {"type": "integer"}, "fail": {"type": "integer"},
            "new": {"type": "integer"}, "missing": {"type": "integer"}, "error": {"type": "integer"}
        },
        "required": ["total", "pass", "fail", "new", "missing", "error"]
    })
}

fn result_output_schema() -> Value {
    json!({
        "type": "object",
        "description": "saccade-result.v1 (schemas/saccade-result.v1.schema.json); with isError, saccade-error.v1.",
        "properties": {
            "schema": {"const": "saccade-result.v1"},
            "verdict": {"enum": ["pass", "regression"]},
            "mode": {"enum": ["regression", "identity"]},
            "totals": totals_schema(),
            "failing": {"type": "array", "items": {
                "type": "object",
                "properties": {
                    "name": {"type": "string"},
                    "status": {"enum": ["fail", "error", "missing", "new"]},
                    "metric": {"enum": ["mean", "p95", "p99", "max"]},
                    "value": {"type": ["number", "null"]},
                    "threshold": {"type": "number"},
                    "error": {"type": "string"},
                    "config_differs": {"type": "array", "items": {"type": "string"}},
                    "failing_regions": {"type": "array", "items": {"type": "string"}},
                    "class": {"enum": ["identical", "noise", "global_tone", "local_structure", "mixed", "misaligned", "broken_frame"]},
                    "description": {"type": "string"},
                    "perf": {"type": "string"},
                    "buffer": {"type": ["object", "null"]},
                    "hotspots": {"type": "array", "items": {"type": "object"}}
                },
                "required": ["name", "status", "metric", "threshold"]
            }},
            "failing_omitted": {"type": "integer"},
            "paths": {"type": "object", "properties": {
                "report_json": {"type": "string"},
                "index_html": {"type": ["string", "null"]},
                "explain_md": {"type": ["string", "null"]}
            }, "required": ["report_json"]},
            "next_step": {"type": "string"},
            "explain_error": {"type": "string"}
        },
        "required": ["schema", "verdict", "totals", "failing", "paths", "next_step"]
    })
}

fn explain_output_schema() -> Value {
    json!({
        "type": "object",
        "description": "saccade-explain-result.v1 (schemas/saccade-explain-result.v1.schema.json); with isError, saccade-error.v1.",
        "properties": {
            "schema": {"const": "saccade-explain-result.v1"},
            "blind": {"type": "boolean"},
            "paths": {"type": "object", "properties": {
                "dir": {"type": "string"},
                "explain_json": {"type": "string"},
                "explain_md": {"type": "string"},
                "blind_key": {"type": ["string", "null"]}
            }, "required": ["dir", "explain_json", "explain_md"]},
            "entries": {"type": "array", "items": {"type": "object"}}
        },
        "required": ["schema", "blind", "paths", "entries"]
    })
}

fn summary_output_schema() -> Value {
    json!({
        "type": "object",
        "description": "saccade-summary.v1 (schemas/saccade-summary.v1.schema.json); with isError, saccade-error.v1.",
        "properties": {
            "schema": {"const": "saccade-summary.v1"},
            "verdict": {"enum": ["pass", "regression"]},
            "totals": totals_schema(),
            "failing": {"type": "array", "items": {"type": "object"}},
            "paths": {"type": "object"}
        },
        "required": ["schema", "verdict", "totals", "failing", "paths"]
    })
}

fn arg_str(args: &Map<String, Value>, key: &str) -> Result<Option<String>, CliError> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) if !s.is_empty() => Ok(Some(s.clone())),
        Some(_) => Err(CliError::usage(format!(
            "`{key}` must be a non-empty string"
        ))),
    }
}

fn require_str(args: &Map<String, Value>, key: &str) -> Result<String, CliError> {
    arg_str(args, key)?.ok_or_else(|| CliError::usage(format!("`{key}` is required")))
}

fn arg_bool(args: &Map<String, Value>, key: &str) -> Result<Option<bool>, CliError> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Bool(b)) => Ok(Some(*b)),
        Some(_) => Err(CliError::usage(format!("`{key}` must be a boolean"))),
    }
}

fn arg_f64(args: &Map<String, Value>, key: &str) -> Result<Option<f64>, CliError> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v
            .as_f64()
            .filter(|n| n.is_finite())
            .map(Some)
            .ok_or_else(|| CliError::usage(format!("`{key}` must be a finite number"))),
    }
}

fn arg_top(args: &Map<String, Value>) -> Result<usize, CliError> {
    match args.get("top") {
        None | Some(Value::Null) => Ok(3),
        Some(v) => v
            .as_u64()
            .filter(|n| (1..=20).contains(n))
            .map(|n| n as usize)
            .ok_or_else(|| CliError::usage("`top` must be an integer from 1 to 20")),
    }
}

fn arg_strings(args: &Map<String, Value>, key: &str) -> Result<Vec<String>, CliError> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::Array(items)) => items
            .iter()
            .map(|v| {
                v.as_str()
                    .map(str::to_owned)
                    .ok_or_else(|| CliError::usage(format!("`{key}` must be an array of strings")))
            })
            .collect(),
        Some(_) => Err(CliError::usage(format!(
            "`{key}` must be an array of strings"
        ))),
    }
}

fn reject_unknown(args: &Map<String, Value>, known: &[&str]) -> Result<(), CliError> {
    match args.keys().find(|k| !known.contains(&k.as_str())) {
        Some(k) => Err(CliError::usage(format!("unknown argument `{k}`"))),
        None => Ok(()),
    }
}

/// The argument names the run tools share.
const RUN_ARGS: &[&str] = &[
    "out_dir",
    "threshold",
    "metric",
    "ppd",
    "labels",
    "meta_name",
    "require_matching_meta",
    "declare",
    "fail_on_new",
    "allow_empty",
    "include_images",
];

/// Applies the arguments `saccade_compare` and `saccade_identity` share.
fn apply_run_args(args: &Map<String, Value>, cfg: &mut RunConfig) -> Result<(), CliError> {
    if let Some(t) = arg_f64(args, "threshold")? {
        cfg.default_threshold = t;
    }
    if let Some(m) = arg_str(args, "metric")? {
        cfg.default_metric = match m.as_str() {
            "mean" => Metric::Mean,
            "p95" => Metric::P95,
            "p99" => Metric::P99,
            "max" => Metric::Max,
            other => {
                return Err(CliError::usage(format!(
                    "`metric` must be mean, p95, p99 or max, got {other:?}"
                )));
            }
        };
    }
    if let Some(p) = arg_f64(args, "ppd")? {
        cfg.pixels_per_degree = p as f32;
    }
    let labels = arg_strings(args, "labels")?;
    match labels.as_slice() {
        [] => {}
        [a, b] if !a.trim().is_empty() && !b.trim().is_empty() => {
            cfg.labels = Labels {
                baseline: a.trim().to_string(),
                capture: b.trim().to_string(),
            };
        }
        _ => {
            return Err(CliError::usage(
                "`labels` takes exactly two non-empty names",
            ));
        }
    }
    if let Some(n) = arg_str(args, "meta_name")? {
        cfg.meta.name = n;
    }
    if let Some(b) = arg_bool(args, "fail_on_new")? {
        cfg.fail_on_new = b;
    }
    if let Some(b) = arg_bool(args, "allow_empty")? {
        cfg.allow_empty = b;
    }
    let required = arg_bool(args, "require_matching_meta")?.unwrap_or(false);
    let declared = arg_strings(args, "declare")?;
    if !declared.is_empty() && !required {
        return Err(CliError::usage(
            "`declare` needs `require_matching_meta: true`",
        ));
    }
    cfg.meta.required |= required;
    cfg.meta.declared.extend(declared);
    Ok(())
}

/// `data` as standard base64.
pub(crate) fn base64(data: &[u8]) -> String {
    const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
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

/// An MCP image content block of the PNG at `path`, downscaled to at most
/// [`MAX_IMAGE_WIDTH`] pixels wide; `None` when it cannot be read.
fn image_block(path: &Path) -> Option<Value> {
    let img = image::open(path).ok()?;
    let img = if img.width() > MAX_IMAGE_WIDTH {
        let h = (f64::from(img.height()) * f64::from(MAX_IMAGE_WIDTH) / f64::from(img.width()))
            .round()
            .max(1.0) as u32;
        img.resize_exact(MAX_IMAGE_WIDTH, h, image::imageops::FilterType::Triangle)
    } else {
        img
    };
    let mut png = Vec::new();
    img.write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png)
        .ok()?;
    Some(json!({"type": "image", "data": base64(&png), "mimeType": "image/png"}))
}

/// The most informative strips of a pack: hotspot strips worst entry first, or
/// the whole-frame strip of an entry that has none.
fn top_strips(pack: &ExplainPack, dir: &Path) -> Vec<Value> {
    let mut images = Vec::new();
    for e in &pack.entries {
        if e.hotspots.is_empty() {
            if let Some(t) = &e.thumbnail {
                images.extend(image_block(&dir.join(t)));
            }
        }
        for h in &e.hotspots {
            images.extend(image_block(&dir.join(&h.strip)));
        }
        if images.len() >= MAX_IMAGES {
            break;
        }
    }
    images.truncate(MAX_IMAGES);
    images
}

impl Server {
    /// A server confined to `root`.
    pub fn new(root: &Path) -> Result<Self, CliError> {
        if !root.is_dir() {
            return Err(CliError::io(format!(
                "--root {} is not a directory",
                root.display()
            )));
        }
        Ok(Self {
            root: absolute(root),
            watches: crate::s6_mcp::Watches::default(),
        })
    }

    /// Resolves an agent-supplied path against the root, canonicalising what
    /// exists, and refuses anything that ends up outside it.
    fn resolve(&self, key: &str, p: &str) -> Result<PathBuf, CliError> {
        let raw = Path::new(p);
        let joined = if raw.is_absolute() {
            raw.to_path_buf()
        } else {
            self.root.join(raw)
        };
        let abs = absolute(&joined);
        if abs.starts_with(&self.root) {
            Ok(abs)
        } else {
            Err(CliError::new(
                "unsafe_path",
                format!(
                    "`{key}`: {} is outside the server root {}",
                    abs.display(),
                    self.root.display()
                ),
            ))
        }
    }

    fn existing_dir(&self, key: &str, p: &str) -> Result<PathBuf, CliError> {
        let path = self.resolve(key, p)?;
        if path.is_dir() {
            Ok(path)
        } else {
            Err(CliError::io(format!(
                "`{key}`: {} is not a directory",
                path.display()
            )))
        }
    }

    fn existing_file(&self, key: &str, p: &str) -> Result<PathBuf, CliError> {
        let path = self.resolve(key, p)?;
        if path.is_file() {
            Ok(path)
        } else {
            Err(CliError::io(format!(
                "`{key}`: {} is not a file",
                path.display()
            )))
        }
    }

    /// Resolves `out_dir` under the root. That it is not inside an input
    /// directory and not a foreign non-empty directory is checked by the run
    /// itself (`saccade_core::run::guard_output_dir`).
    fn checked_out_dir(&self, out: &str, _inputs: &[&Path]) -> Result<PathBuf, CliError> {
        self.resolve("out_dir", out)
    }

    /// Runs a comparison and, when something fails, writes the explain pack.
    fn run_and_explain(
        &self,
        (baseline, capture, out): (&Path, &Path, &Path),
        cfg: &RunConfig,
        include_images: bool,
    ) -> ToolResult {
        cfg.validate()?;
        let report: Report = saccade_core::run::run(baseline, capture, out, cfg)?;
        let report_json = out.join(saccade_core::report::REPORT_FILE_NAME);
        if let Some(e) = nothing_compared(&report, &report_json) {
            return Err(e);
        }
        let explain_dir = out.join("explain");
        let mut explain_error = None;
        let mut images = Vec::new();
        let mut written = false;
        if report.totals.fail > 0 {
            match explain(&report_json, &explain_dir, &ExplainOptions::default()) {
                Ok(pack) => {
                    written = true;
                    if include_images {
                        images = top_strips(&pack, &explain_dir);
                    }
                }
                Err(e) => explain_error = Some(e.to_string()),
            }
        }
        let mut value = result_value(&report, &report_json, DEFAULT_TOP_FAILING, written);
        if let (Some(err), Some(obj)) = (explain_error, value.as_object_mut()) {
            obj.insert("explain_error".into(), Value::String(err));
        }
        let text = summary_text(&report, &value);
        Ok(ToolOutput {
            structured: value,
            text,
            images,
        })
    }

    fn tool_compare(&self, args: &Map<String, Value>) -> ToolResult {
        let mut known = vec!["baseline_dir", "capture_dir", "config"];
        known.extend_from_slice(RUN_ARGS);
        reject_unknown(args, &known)?;
        let baseline = self.existing_dir("baseline_dir", &require_str(args, "baseline_dir")?)?;
        let capture = self.existing_dir("capture_dir", &require_str(args, "capture_dir")?)?;
        let out = self.checked_out_dir(&require_str(args, "out_dir")?, &[&baseline, &capture])?;
        let mut cfg = match arg_str(args, "config")? {
            Some(p) => RunConfig::from_toml_file(&self.existing_file("config", &p)?)?,
            None => {
                let auto = self.root.join("saccade.toml");
                if auto.is_file() {
                    RunConfig::from_toml_file(&auto)?
                } else {
                    RunConfig::default()
                }
            }
        };
        apply_run_args(args, &mut cfg)?;
        let images = arg_bool(args, "include_images")?.unwrap_or(true);
        self.run_and_explain((&baseline, &capture, &out), &cfg, images)
    }

    fn sequence_rank_config(&self, args: &Map<String, Value>) -> Result<RunConfig, CliError> {
        let config = match arg_str(args, "config")? {
            Some(p) => Some(self.existing_file("config", &p)?),
            None if self.root.join("saccade.toml").is_file() => {
                Some(self.existing_file("config", "saccade.toml")?)
            }
            None => None,
        };
        let mut cfg = match config {
            Some(p) => RunConfig::from_toml_file(&p)?,
            None => RunConfig::default(),
        };
        let mut run_args = args.clone();
        run_args.remove("labels");
        apply_run_args(&run_args, &mut cfg)?;
        if let Some(t) = arg_str(args, "hdr_tonemapper")? {
            cfg.hdr.tonemapper = saccade_core::hdr::Tonemapper::parse(&t)?;
        }
        if let Some(e) = arg_str(args, "hdr_exposures")? {
            cfg.hdr.parse_exposures(&e)?;
        }
        Ok(cfg)
    }

    fn tool_sequence(&self, args: &Map<String, Value>) -> ToolResult {
        let mut known: Vec<_> = RUN_ARGS
            .iter()
            .copied()
            .filter(|&k| k != "include_images")
            .collect();
        known.extend([
            "baseline_dir",
            "capture_dir",
            "config",
            "pattern",
            "hdr_tonemapper",
            "hdr_exposures",
        ]);
        reject_unknown(args, &known)?;
        let baseline = self.existing_dir("baseline_dir", &require_str(args, "baseline_dir")?)?;
        let capture = self.existing_dir("capture_dir", &require_str(args, "capture_dir")?)?;
        let out = self.resolve("out_dir", &require_str(args, "out_dir")?)?;
        let mut cfg = self.sequence_rank_config(args)?;
        let labels = arg_strings(args, "labels")?;
        if !labels.is_empty() {
            cfg.labels = crate::parse_labels(&labels)?;
        }
        let pattern = arg_str(args, "pattern")?.unwrap_or_else(|| "*".into());
        let report =
            saccade_core::sequence::run_sequence(&baseline, &capture, &out, &pattern, &cfg)?;
        let mut structured = serde_json::to_value(report.lean())?;
        round_floats(&mut structured);
        Ok(ToolOutput {
            structured,
            text: report.text(),
            images: Vec::new(),
        })
    }

    fn tool_rank(&self, args: &Map<String, Value>) -> ToolResult {
        let mut known: Vec<_> = RUN_ARGS
            .iter()
            .copied()
            .filter(|&k| k != "include_images")
            .collect();
        known.extend([
            "reference_dir",
            "candidate_dirs",
            "config",
            "hdr_tonemapper",
            "hdr_exposures",
        ]);
        reject_unknown(args, &known)?;
        let reference = self.existing_dir("reference_dir", &require_str(args, "reference_dir")?)?;
        let candidates = arg_strings(args, "candidate_dirs")?
            .iter()
            .map(|p| self.existing_dir("candidate_dirs", p))
            .collect::<Result<Vec<_>, _>>()?;
        if candidates.is_empty() {
            return Err(CliError::usage(
                "candidate_dirs requires at least one directory",
            ));
        }
        let out = self.resolve("out_dir", &require_str(args, "out_dir")?)?;
        let cfg = self.sequence_rank_config(args)?;
        let labels = arg_strings(args, "labels")?;
        let metric = if args.contains_key("metric") {
            cfg.default_metric
        } else {
            Metric::Mean
        };
        let report = saccade_core::rank::run_rank(
            &reference,
            &candidates,
            (!labels.is_empty()).then_some(labels.as_slice()),
            metric,
            &out,
            &cfg,
        )?;
        let mut structured = serde_json::to_value(report.lean())?;
        round_floats(&mut structured);
        Ok(ToolOutput {
            structured,
            text: report.text(),
            images: Vec::new(),
        })
    }

    fn tool_identity(&self, args: &Map<String, Value>) -> ToolResult {
        let mut known = vec!["parent_dir", "candidate_dir"];
        known.extend_from_slice(RUN_ARGS);
        reject_unknown(args, &known)?;
        let parent = self.existing_dir("parent_dir", &require_str(args, "parent_dir")?)?;
        let candidate = self.existing_dir("candidate_dir", &require_str(args, "candidate_dir")?)?;
        let out = self.checked_out_dir(&require_str(args, "out_dir")?, &[&parent, &candidate])?;
        let mut cfg = RunConfig {
            mode: Mode::Identity,
            labels: Labels {
                baseline: "parent".into(),
                capture: "candidate".into(),
            },
            default_threshold: 0.0,
            default_metric: Metric::Max,
            ..RunConfig::default()
        };
        apply_run_args(args, &mut cfg)?;
        let images = arg_bool(args, "include_images")?.unwrap_or(true);
        self.run_and_explain((&parent, &candidate, &out), &cfg, images)
    }

    fn tool_explain(&self, args: &Map<String, Value>) -> ToolResult {
        reject_unknown(
            args,
            &[
                "report_json",
                "out_dir",
                "top",
                "hotspot_min_share",
                "blind",
                "key_out",
                "include_images",
            ],
        )?;
        let report_json = self.existing_file("report_json", &require_str(args, "report_json")?)?;
        let out = self.resolve("out_dir", &require_str(args, "out_dir")?)?;
        let blind = arg_bool(args, "blind")?.unwrap_or(false);
        let key_out = match arg_str(args, "key_out")? {
            Some(k) => Some(self.resolve("key_out", &k)?),
            None => None,
        };
        if blind && key_out.is_none() {
            return Err(CliError::usage("`blind` needs `key_out`"));
        }
        if !blind && key_out.is_some() {
            return Err(CliError::usage("`key_out` needs `blind: true`"));
        }
        if let Some(k) = &key_out {
            saccade_core::explain::check_key_out(&out, k).map_err(|e| {
                CliError::new(
                    "unsafe_path",
                    e.to_string().replace("invalid configuration: ", ""),
                )
            })?;
        }
        let mut opts = ExplainOptions {
            top: arg_top(args)?,
            blind,
            key_out: key_out.clone(),
            ..ExplainOptions::default()
        };
        if let Some(s) = arg_f64(args, "hotspot_min_share")? {
            if !(0.0..=1.0).contains(&s) {
                return Err(CliError::usage("`hotspot_min_share` must be from 0 to 1"));
            }
            opts.hotspot_min_share = s;
        }
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
        let mut value = json!({
            "schema": "saccade-explain-result.v1",
            "blind": blind,
            "paths": {
                "dir": out.display().to_string(),
                "explain_json": abs(saccade_core::explain::EXPLAIN_FILE),
                "explain_md": abs(saccade_core::explain::EXPLAIN_MD_FILE),
                "blind_key": key_out.as_ref().map(|k| k.display().to_string()),
            },
            "entries": entries,
        });
        round_floats(&mut value);
        let text = format!(
            "saccade explain: {}, {}{}\nsummary: {}",
            saccade_core::explain::plural(pack.entries.len(), "entry", "entries"),
            saccade_core::explain::plural(strips, "hotspot strip", "hotspot strips"),
            if blind {
                " (blind; key written to key_out, keep it from the judge)"
            } else {
                ""
            },
            abs(saccade_core::explain::EXPLAIN_MD_FILE)
        );
        let images = if arg_bool(args, "include_images")?.unwrap_or(true) {
            top_strips(&pack, &out)
        } else {
            Vec::new()
        };
        Ok(ToolOutput {
            structured: value,
            text,
            images,
        })
    }

    fn tool_summary(&self, args: &Map<String, Value>) -> ToolResult {
        reject_unknown(args, &["report_json"])?;
        let path = self.existing_file("report_json", &require_str(args, "report_json")?)?;
        let text = std::fs::read_to_string(&path)
            .map_err(|e| CliError::io(format!("reading {}: {e}", path.display())))?;
        let report: Report = serde_json::from_str(&text)
            .map_err(|e| CliError::io(format!("parsing {}: {e}", path.display())))?;
        let mut value = summary_value(&report, &path, DEFAULT_TOP_FAILING);
        round_floats(&mut value);
        let text = summary_text(&report, &value);
        Ok(ToolOutput {
            structured: value,
            text,
            images: Vec::new(),
        })
    }

    fn tool_compare_runs(&self, args: &Map<String, Value>) -> ToolResult {
        use saccade_core::runs::{
            NoAssets, Pairing, RunInput, RunsOptions, overview, unique_labels,
        };
        reject_unknown(
            args,
            &[
                "ref_dir",
                "run_dirs",
                "labels",
                "pair_by_position",
                "ppd",
                "meta_name",
            ],
        )?;
        let mut dirs = vec![self.existing_dir("ref_dir", &require_str(args, "ref_dir")?)?];
        let given = arg_strings(args, "run_dirs")?;
        if given.is_empty() || given.len() > saccade_core::runs::MAX_RUNS {
            return Err(CliError::usage(format!(
                "`run_dirs` takes 1 to {} directories",
                saccade_core::runs::MAX_RUNS
            )));
        }
        for g in &given {
            dirs.push(self.existing_dir("run_dirs", g)?);
        }
        let labels = match arg_strings(args, "labels")? {
            l if l.is_empty() => unique_labels(&dirs),
            l if l.len() == dirs.len() => l,
            l => {
                return Err(CliError::usage(format!(
                    "`labels` takes one label per directory ({}), got {}",
                    dirs.len(),
                    l.len()
                )));
            }
        };
        let by_position = arg_bool(args, "pair_by_position")?.unwrap_or(false);
        let mut opts = RunsOptions::default();
        if let Some(p) = arg_f64(args, "ppd")? {
            opts.pixels_per_degree = p as f32;
        }
        if let Some(n) = arg_str(args, "meta_name")? {
            opts.meta.name = n;
        }
        let mut inputs: Vec<RunInput> = dirs
            .into_iter()
            .zip(labels)
            .enumerate()
            .map(|(i, (dir, label))| RunInput {
                display: dir.display().to_string(),
                dir,
                label,
                pairing: if by_position && i > 0 {
                    Pairing::Position
                } else {
                    Pairing::Name
                },
            })
            .collect();
        let reference = inputs.remove(0);
        let model = overview(&reference, &inputs, &opts, None, &NoAssets)?;
        let mut value = crate::runs_cmd::lean_value(&model)?;
        round_floats(&mut value);
        Ok(ToolOutput {
            structured: value,
            text: crate::runs_cmd::summary_text(&model),
            images: Vec::new(),
        })
    }

    fn call_tool(&self, name: &str, args: &Map<String, Value>) -> Option<ToolResult> {
        if let Some(result) = crate::judge_cmd::mcp_call(name, args, &|key, p| self.resolve(key, p))
        {
            return Some(result.map(|(structured, text)| ToolOutput {
                structured,
                text,
                images: Vec::new(),
            }));
        }
        if let Some(result) =
            crate::s6_mcp::call(name, args, &self.watches, &|key, p| self.resolve(key, p))
        {
            return Some(result.map(|(structured, text)| ToolOutput {
                structured,
                text,
                images: Vec::new(),
            }));
        }
        Some(match name {
            "saccade_compare_runs" => self.tool_compare_runs(args),
            "saccade_sequence" => self.tool_sequence(args),
            "saccade_rank" => self.tool_rank(args),
            "saccade_compare" => self.tool_compare(args),
            "saccade_identity" => self.tool_identity(args),
            "saccade_explain" => self.tool_explain(args),
            "saccade_summary" => self.tool_summary(args),
            _ => {
                let resolved =
                    crate::agent_ui::mcp_call(name, args, &|key, p| self.resolve(key, p))?;
                resolved.map(|(structured, text, images)| ToolOutput {
                    structured,
                    text,
                    images,
                })
            }
        })
    }

    /// Handles one JSON-RPC message; `None` for notifications (no reply).
    pub fn handle_message(&self, msg: &Value) -> Option<Value> {
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
        if method == "notifications/initialized" {
            self.watches.activate();
        }
        // No id: a notification such as notifications/initialized, never answered.
        let id = id?;
        let params = obj.get("params").and_then(Value::as_object);
        let result = match method {
            "initialize" => json!({
                "protocolVersion": PROTOCOL_VERSION,
                "capabilities": {"tools": {"listChanged": false}, "logging": {}},
                "serverInfo": {"name": "saccade", "version": env!("CARGO_PKG_VERSION")},
                "instructions": format!("Perceptual (FLIP) image regression. Call saccade_compare (or saccade_identity for a no-pixel-change refactor): it writes a report plus hotspot crops, attaches the top strips as images, and returns structuredContent.next_step. Read failing[].hotspots for where the visible difference is. Every path must be under the server root ({}); relative paths resolve against it.", self.root.display()),
            }),
            "ping" => json!({}),
            "tools/list" => {
                let mut tools = tool_schemas();
                if let Some(list) = tools.as_array_mut() {
                    list.extend(crate::agent_ui::mcp_schemas());
                    list.extend(crate::judge_cmd::mcp_schemas());
                    list.extend(crate::s6_mcp::schemas());
                }
                json!({"tools": tools})
            }
            "tools/call" => {
                let Some(name) = params.and_then(|p| p.get("name")).and_then(Value::as_str) else {
                    return Some(rpc_error(id, -32602, "tools/call needs a string `name`"));
                };
                let empty = Map::new();
                let args = match params.and_then(|p| p.get("arguments")) {
                    None | Some(Value::Null) => &empty,
                    Some(Value::Object(a)) => a,
                    Some(_) => {
                        return Some(rpc_error(id, -32602, "`arguments` must be an object"));
                    }
                };
                match self.call_tool(name, args) {
                    Some(r) => tool_response(r),
                    None => return Some(rpc_error(id, -32602, &format!("unknown tool {name:?}"))),
                }
            }
            other => return Some(rpc_error(id, -32601, &format!("method not found: {other}"))),
        };
        Some(json!({"jsonrpc": "2.0", "id": id, "result": result}))
    }
}

fn tool_response(result: ToolResult) -> Value {
    match result {
        Ok(out) => {
            let mut content = vec![json!({"type": "text", "text": out.text})];
            content.extend(out.images);
            json!({
                "content": content,
                "structuredContent": out.structured,
                "isError": false,
            })
        }
        Err(e) => json!({
            "content": [{"type": "text", "text": format!("saccade error [{}]: {}", e.code, e.message)}],
            "structuredContent": e.value(),
            "isError": true,
        }),
    }
}

fn rpc_error(id: Value, code: i64, message: &str) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
}

/// Serves MCP on stdin/stdout until stdin closes. `root` defaults to the
/// working directory.
pub fn serve_stdio(root: Option<&Path>, watch: &[String]) -> Result<(), CliError> {
    let root = match root {
        Some(r) => r.to_path_buf(),
        None => std::env::current_dir()
            .map_err(|e| CliError::io(format!("reading the working directory: {e}")))?,
    };
    let mut server = Server::new(&root)?;
    let mut watches = crate::s6_mcp::Watches::default();
    watches.start(watch, &|key, p| server.resolve(key, p))?;
    server.watches = watches;
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    for line in stdin.lock().lines() {
        let line = line.map_err(|e| CliError::io(format!("reading stdin: {e}")))?;
        if line.trim().is_empty() {
            continue;
        }
        let reply = match serde_json::from_str::<Value>(&line) {
            Ok(msg) => server.handle_message(&msg),
            Err(e) => Some(rpc_error(Value::Null, -32700, &format!("parse error: {e}"))),
        };
        if let Some(reply) = reply {
            let mut out = stdout.lock();
            let text = serde_json::to_string(&reply)?;
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
