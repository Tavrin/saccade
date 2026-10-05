//! Six discriminated MCP tools over newline-delimited JSON-RPC 2.0.
//! Capture roots are read-only. Generated files require a separate out-root;
//! shared RootPolicy containment checks aliases and transitive references.
//! Provider review requires explicit startup authorization and shared budgets. Images require include_images=true,
//! are capped at three blocks and 1024 pixels wide, and confer no authority.

use std::io::{BufRead, Cursor, Write};
use std::path::{Path, PathBuf};

use saccade_core::config::RunConfig;
use saccade_core::explain::{ExplainOptions, ExplainPack, explain};
use saccade_core::report::{Labels, Metric, Mode, Report};
use serde_json::{Map, Value, json};

#[cfg(feature = "graphics")]
use crate::agent::round_floats;
use crate::agent::{CliError, DEFAULT_TOP_FAILING, result_value};

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
    policy: saccade_core::root_policy::RootPolicy,
    #[cfg(feature = "ai")]
    providers: crate::review_cmd::Startup,
    #[cfg(feature = "ai")]
    run_id: String,
}

fn measurement_schemas() -> Value {
    let run_props = |baseline: &str, capture: &str| {
        json!({
            "out_dir":{"type":"string","minLength":1},
            "threshold":{"type":"number","minimum":0,"maximum":1},
            "metric":{"type":"string","enum":["mean","p95","p99","max"]},
            "ppd":{"type":"number","exclusiveMinimum":0},
            "labels":{"type":"array","items":{"type":"string"},"default":[baseline,capture]},
            "meta_name":{"type":"string"},
            "require_matching_meta":{"type":"boolean","default":false},
            "declare":{"type":"array","items":{"type":"string"}},
            "fail_on_new":{"type":"boolean","default":true},
            "allow_empty":{"type":"boolean","default":false},
            "include_images":{"type":"boolean","default":false},
            "entries":{"type":"array","items":{"type":"string"}},
            "record_absolute_paths":{"type":"boolean","default":false},
            "perf_name":{"type":"string"},"perf_noise":{"type":"string"},
            "perf_noise_k":{"type":"number","exclusiveMinimum":0},
            "perf_resolution_ms":{"type":"number","exclusiveMinimum":0},
            "perf_resolution_ticks":{"type":"integer","minimum":1},
            "perf_min_delta_ms":{"type":"number","minimum":0},
            "perf_min_delta_pct":{"type":"number","minimum":0}
        })
    };
    let dir = |what: &str| json!({"type": "string", "minLength": 1, "description": what});
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
            "operation":"ablate", "title":"Compare ablation arms",
            "description":"One row per arm against a common base: image identity/FLIP class, frame and beyond-noise term changes, configuration differences, NO-EFFECT and PERF-ONLY flags, and a combined verdict. Writes HTML, JSON and text evidence.",
            "inputSchema":{"type":"object","properties":{
                "base_dir":dir("Base capture directory"),
                "arm_dirs":{"type":"array","items":{"type":"string","minLength":1},"minItems":1},
                "out_dir":dir("Output directory"), "config":{"type":"string"},
                "top":{"type":"integer","minimum":0,"default":5},
                "perf_name":{"type":"string"}, "perf_noise":{"type":"string"},
                "perf_noise_k":{"type":"number","exclusiveMinimum":0,"default":3},
                "perf_resolution_ms":{"type":"number","exclusiveMinimum":0},
                "perf_resolution_ticks":{"type":"integer","minimum":1,"default":2},
                "perf_min_delta_ms":{"type":"number","minimum":0,"default":0.05},
                "perf_min_delta_pct":{"type":"number","minimum":0,"default":0.5},
                "record_absolute_paths":{"type":"boolean","default":false}
            },"required":["base_dir","arm_dirs","out_dir"],"additionalProperties":false},
            "annotations":{"readOnlyHint":false,"destructiveHint":false,"idempotentHint":true,"openWorldHint":false}
        },
        {
            "operation": "sequence",
            "title": "Compare numbered frame sequences",
            "description": "Pair colour frames by numeric sorted index, measure per-frame FLIP and added temporal instability (capture consecutive-frame mean minus baseline). Writes saccade-sequence.v1.json and a normal per-frame HTML report with a server-rendered SVG curve. Returns a lean saccade-sequence.v1; full frame details stay on disk. Same exit/verdict rules as compare; temporal decode errors are regressions.",
            "inputSchema": {"type": "object", "properties": sequence_props, "required": ["baseline_dir", "capture_dir", "out_dir"], "additionalProperties": false},
            "annotations": {"readOnlyHint": false, "destructiveHint": false, "idempotentHint": true, "openWorldHint": false}
        },
        {
            "operation": "rank",
            "title": "Rank candidate image directories",
            "description": "Compare each candidate directory against one reference using mean, p95, p99 or max FLIP. Writes rankings, Markdown tables and one normal report per candidate under out_dir/label. Returns lean saccade-rank.v1 with overall competition ranks, mean ranks/metrics, bit-identical counts and report paths. Missing/error comparisons cannot win an overall ranking.",
            "inputSchema": {"type": "object", "properties": rank_props, "required": ["reference_dir", "candidate_dirs", "out_dir"], "additionalProperties": false},
            "annotations": {"readOnlyHint": false, "destructiveHint": false, "idempotentHint": true, "openWorldHint": false}
        },
        {
            "operation": "compare",
            "title": "Compare captures against baselines",
            "description": "Start here for a visual-regression check. Compares every image in capture_dir with the same-named image in baseline_dir using NVIDIA FLIP (a perceptual metric: invisible differences score ~0, visible ones score high), writes the report (index.html, saccade-report.v1.json, heatmaps) to out_dir and, when anything fails, an explain pack with hotspot crops to out_dir/explain. A regression is a normal result (verdict \"regression\"), not an error. Read structuredContent.failing[] (value vs threshold, hotspots = where the error is concentrated, config_differs = capture-setup keys that changed), then the attached images, then follow structuredContent.next_step. Re-running with the same arguments is safe.",
            "inputSchema": {
                "type": "object",
                "properties": compare_props,
                "required": ["baseline_dir", "capture_dir", "out_dir"],
                "additionalProperties": false
            },
            "annotations": {"title": "Compare captures against baselines", "readOnlyHint": false, "destructiveHint": false, "idempotentHint": true, "openWorldHint": false}
        },
        {
            "operation": "identity",
            "title": "Check a candidate build against its parent",
            "description": "Use after a change that should not alter any pixel (a refactor, an optimisation): strict defaults (metric max, threshold 0), and bit-identity is reported per image. Same output and next steps as saccade_compare. No config file is read.",
            "inputSchema": {
                "type": "object",
                "properties": identity_props,
                "required": ["parent_dir", "candidate_dir", "out_dir"],
                "additionalProperties": false
            },
            "annotations": {"title": "Check a candidate build against its parent", "readOnlyHint": false, "destructiveHint": false, "idempotentHint": true, "openWorldHint": false}
        },
    ])
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
    if key == "artifact"
        && let Some(reference) = args.get(key).and_then(Value::as_object)
    {
        if reference.len() != 2 || !reference.contains_key("sha256") {
            return Err(CliError::usage(
                "artifact reference requires path and sha256",
            ));
        }
        return reference
            .get("path")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| CliError::usage("artifact path must be a string"));
    }
    arg_str(args, key)?.ok_or_else(|| CliError::usage(format!("missing {key}")))
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
    "perf_name",
    "perf_noise",
    "perf_noise_k",
    "perf_resolution_ms",
    "perf_resolution_ticks",
    "perf_min_delta_ms",
    "perf_min_delta_pct",
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
    "entries",
    "record_absolute_paths",
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
    cfg.entries = arg_strings(args, "entries")?;
    cfg.record_absolute_paths = arg_bool(args, "record_absolute_paths")?.unwrap_or(false);
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
        if e.hotspots.is_empty()
            && let Some(t) = &e.thumbnail
        {
            images.extend(image_block(&dir.join(t)));
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
    /// Build a local transport from human-controlled startup permissions.
    pub fn new(
        roots: &[PathBuf],
        output: Option<&Path>,
        follow: bool,
        targets: &[PathBuf],
    ) -> Result<Self, CliError> {
        let policy = saccade_core::root_policy::RootPolicy::new(roots, output, follow, targets)?;
        Ok(Self {
            root: policy.roots[0].path.clone(),
            policy,
            #[cfg(feature = "ai")]
            providers: Default::default(),
            #[cfg(feature = "ai")]
            run_id: saccade_core::budget_ledger::new_id(),
        })
    }
    fn resolve(&self, key: &str, path: &str) -> Result<PathBuf, CliError> {
        let write = matches!(key, "out" | "out_dir" | "key_out");
        let result = if write {
            self.policy.write(Path::new(path))
        } else {
            self.policy.read(Path::new(path))
        };
        result.map_err(|e| CliError::new("unsafe_path", format!("{key}: {e}")))
    }
    /// Resolve transitive file references relative to their owning document.
    fn document_inputs(&self, path: &Path) -> Result<(), CliError> {
        let value = crate::local_cmd::read_value(path)?;
        self.check_references(&value, path.parent().unwrap_or(Path::new(".")), "")?;
        if value["schema"] == saccade_core::report::REPORT_SCHEMA {
            let report: Report = serde_json::from_value(value)?;
            for entry in &report.entries {
                for (baseline, hash) in [
                    (true, &entry.baseline_sha256),
                    (false, &entry.capture_sha256),
                ] {
                    if let (Some(file), Some(hash)) = (
                        crate::local_cmd::report_input(&report, path, &entry.name, baseline),
                        hash,
                    ) {
                        let file = self
                            .policy
                            .read(&file)
                            .map_err(|e| CliError::new("unsafe_path", e.to_string()))?;
                        if saccade_core::run::sha256_file(&file)? != *hash {
                            return Err(CliError::new("stale_evidence", "measured input changed"));
                        }
                    }
                }
            }
        }
        Ok(())
    }
    fn check_references(&self, value: &Value, base: &Path, context: &str) -> Result<(), CliError> {
        match value {
            Value::Array(values) => {
                for value in values {
                    self.check_references(value, base, context)?;
                }
            }
            Value::Object(map) => {
                for (key, value) in map {
                    if (context == "paths"
                        && matches!(key.as_str(), "baseline" | "capture" | "heatmap")
                        || matches!(
                            key.as_str(),
                            "path" | "baseline_dir" | "capture_dir" | "view_dir" | "report_json"
                        ))
                        && let Some(path) = value.as_str()
                    {
                        let resolved = saccade_core::paths::native(Path::new(path));
                        let joined = if resolved.is_absolute() {
                            resolved.into_owned()
                        } else {
                            base.join(resolved)
                        };
                        let path = self.policy.read(&joined).map_err(|e| {
                            CliError::new("unsafe_path", format!("referenced {key}: {e}"))
                        })?;
                        if let Some(hash) = map.get("sha256").and_then(Value::as_str) {
                            let actual = saccade_core::run::sha256_file(&path)?;
                            if actual != hash.strip_prefix("sha256:").unwrap_or(hash) {
                                return Err(CliError::new(
                                    "stale_evidence",
                                    "referenced content hash changed",
                                ));
                            }
                        }
                    }
                    self.check_references(value, base, key)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
    /// Validate descendants before computations open images, masks or sidecars.
    fn input_tree(&self, path: &Path) -> Result<(), CliError> {
        if path.is_file() {
            self.policy
                .read(path)
                .map_err(|e| CliError::new("unsafe_path", e.to_string()))?;
            return Ok(());
        }
        let mut queue = vec![(path.to_owned(), 0)];
        let mut seen = std::collections::HashSet::new();
        while let Some((dir, depth)) = queue.pop() {
            if depth > 64 {
                return Err(CliError::new("unsafe_path", "input tree exceeds 64 levels"));
            }
            let canonical =
                saccade_core::paths::canonicalize(&dir).map_err(|e| CliError::io(e.to_string()))?;
            if !seen.insert(canonical) {
                continue;
            }
            for child in std::fs::read_dir(dir).map_err(|e| CliError::io(e.to_string()))? {
                let child = child.map_err(|e| CliError::io(e.to_string()))?.path();
                let allowed = self
                    .policy
                    .read(&child)
                    .map_err(|e| CliError::new("unsafe_path", e.to_string()))?;
                if allowed.is_dir() {
                    queue.push((allowed, depth + 1));
                }
            }
        }
        Ok(())
    }
    fn validate_config(&self, cfg: &RunConfig) -> Result<(), CliError> {
        if !cfg.symlink_targets.is_empty() {
            return Err(CliError::new(
                "unsafe_path",
                "project settings cannot authorize symlink targets",
            ));
        }
        for mask in &cfg.masks {
            if let Some(path) = &mask.image {
                self.policy
                    .read(&cfg.config_dir.as_deref().unwrap_or(&self.root).join(path))
                    .map_err(|e| CliError::new("unsafe_path", e.to_string()))?;
            }
        }
        if let Some(path) = &cfg.perf.noise {
            self.policy
                .read(path)
                .map_err(|e| CliError::new("unsafe_path", e.to_string()))?;
        }
        cfg.meta.checker()?;
        Ok(())
    }

    fn output_paths(&self, value: &mut Value, absolute_paths: bool) {
        if let Some(reference) = value.get_mut("artifact")
            && let Some(path) = reference.get_mut("path")
            && let Some(s) = path.as_str()
        {
            *path = json!(saccade_core::paths::record(
                &saccade_core::explain::absolute(Path::new(s)),
                &self.root,
                absolute_paths
            ));
        }
        if let Some(actions) = value.get_mut("next_actions").and_then(Value::as_array_mut) {
            for action in actions {
                self.output_paths(&mut action["arguments"], absolute_paths);
            }
        }
        for key in [
            "report_json",
            "frames_report_json",
            "index_html",
            "markdown",
        ] {
            if let Some(path) = value.get_mut(key)
                && let Some(s) = path.as_str()
            {
                *path = Value::String(saccade_core::paths::record(
                    &saccade_core::explain::absolute(Path::new(s)),
                    &self.root,
                    absolute_paths,
                ));
            }
        }
        if let Some(overall) = value.get_mut("overall").and_then(Value::as_array_mut) {
            for candidate in overall {
                self.output_paths(candidate, absolute_paths);
            }
        }

        if let Some(paths) = value.get_mut("paths").and_then(Value::as_object_mut) {
            for path in paths.values_mut() {
                if let Some(s) = path.as_str() {
                    *path = Value::String(saccade_core::paths::record(
                        &saccade_core::explain::absolute(Path::new(s)),
                        &self.root,
                        absolute_paths,
                    ));
                }
            }
        }
    }

    fn existing_dir(&self, key: &str, p: &str) -> Result<PathBuf, CliError> {
        let path = self.resolve(key, p)?;
        if path.is_dir() {
            self.input_tree(&path)?;
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
        self.validate_config(cfg)?;
        self.input_tree(baseline)?;
        self.input_tree(capture)?;
        let mut report: Report = saccade_core::run::run(baseline, capture, out, cfg)?;
        // Retain the authorized alias route in references so downstream reads
        // do not turn a registered target into an independently browsable root.
        let base = out;
        report.baseline_dir = Some(crate::local_cmd::lexical_record(baseline, base));
        report.capture_dir = Some(crate::local_cmd::lexical_record(capture, base));
        std::fs::write(
            out.join(saccade_core::report::REPORT_FILE_NAME),
            serde_json::to_vec_pretty(&report)?,
        )
        .map_err(|e| CliError::io(e.to_string()))?;
        let report_json = out.join(saccade_core::report::REPORT_FILE_NAME);
        crate::local_cmd::persist_case(&report, &report_json, &crate::IntentArgs::default())?;
        let explain_dir = out.join("explain");
        let mut explain_error = None;
        let mut images = Vec::new();
        let mut written = false;
        if report.totals.fail > 0 {
            match explain(
                &report_json,
                &explain_dir,
                &ExplainOptions {
                    record_absolute_paths: cfg.record_absolute_paths,
                    ..Default::default()
                },
            ) {
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
        let text = "Measurement completed.".into();
        Ok(ToolOutput {
            structured: value,
            text,
            images,
        })
    }

    fn apply_perf_args(
        &self,
        args: &Map<String, Value>,
        opts: &mut saccade_core::perf::PerfOptions,
    ) -> Result<(), CliError> {
        if let Some(n) = arg_str(args, "perf_name")? {
            opts.name = n;
        }
        if let Some(n) = arg_str(args, "perf_noise")? {
            opts.noise = Some(self.existing_file("perf_noise", &n)?);
        }
        if let Some(k) = arg_f64(args, "perf_noise_k")? {
            opts.k = k;
        }
        if let Some(v) = arg_f64(args, "perf_resolution_ms")? {
            opts.resolution_ms = Some(v);
        }
        if let Some(v) = arg_f64(args, "perf_min_delta_ms")? {
            opts.min_delta_ms = Some(v);
        }
        if let Some(v) = arg_f64(args, "perf_min_delta_pct")? {
            opts.min_delta_pct = Some(v);
        }
        if let Some(v) = args.get("perf_resolution_ticks") {
            opts.resolution_ticks = Some(
                v.as_u64()
                    .and_then(|v| u32::try_from(v).ok())
                    .ok_or_else(|| {
                        CliError::usage("perf_resolution_ticks must be a positive 32-bit integer")
                    })?,
            );
        }
        // Config-provided paths obey the same server confinement as explicit arguments.
        if let Some(n) = &opts.noise {
            opts.noise = Some(self.existing_file("perf_noise", &saccade_core::paths::portable(n))?);
        }
        opts.validate()?;
        Ok(())
    }

    #[cfg(feature = "graphics")]
    fn tool_ablate(&self, args: &Map<String, Value>) -> ToolResult {
        reject_unknown(
            args,
            &[
                "base_dir",
                "arm_dirs",
                "out_dir",
                "config",
                "top",
                "perf_name",
                "perf_noise",
                "perf_noise_k",
                "perf_resolution_ms",
                "perf_resolution_ticks",
                "perf_min_delta_ms",
                "perf_min_delta_pct",
                "record_absolute_paths",
            ],
        )?;
        let base = self.existing_dir("base_dir", &require_str(args, "base_dir")?)?;
        let arms = arg_strings(args, "arm_dirs")?
            .iter()
            .map(|p| self.existing_dir("arm_dirs", p))
            .collect::<Result<Vec<_>, _>>()?;
        let out = self.resolve("out_dir", &require_str(args, "out_dir")?)?;
        let mut cfg = match arg_str(args, "config")? {
            Some(p) => RunConfig::from_toml_file(&self.existing_file("config", &p)?)?,
            None => RunConfig::default(),
        };
        cfg.record_absolute_paths = arg_bool(args, "record_absolute_paths")?.unwrap_or(false);
        self.apply_perf_args(args, &mut cfg.perf)?;
        let top = match args.get("top") {
            Some(v) => v
                .as_u64()
                .and_then(|v| usize::try_from(v).ok())
                .ok_or_else(|| CliError::usage("top must be a nonnegative integer"))?,
            None => 5,
        };
        self.validate_config(&cfg)?;
        let model = saccade_core::ablate::run(&base, &arms, &out, &cfg, top)?;
        Ok(ToolOutput {
            structured: serde_json::to_value(&model)?,
            text: model.text(),
            images: Vec::new(),
        })
    }

    fn tool_compare(&self, args: &Map<String, Value>) -> ToolResult {
        let mut known = vec!["baseline_dir", "capture_dir", "config"];
        known.extend_from_slice(RUN_ARGS);
        reject_unknown(args, &known)?;
        let baseline = self.resolve("baseline_dir", &require_str(args, "baseline_dir")?)?;
        let capture = self.resolve("capture_dir", &require_str(args, "capture_dir")?)?;
        let out = self.checked_out_dir(&require_str(args, "out_dir")?, &[&baseline, &capture])?;
        let mut cfg = match arg_str(args, "config")? {
            Some(p) => RunConfig::from_toml_file(&self.existing_file("config", &p)?)?,
            None => {
                let auto = self.root.join("saccade.toml");
                if auto.is_file() {
                    RunConfig::from_toml_file(
                        &self.existing_file("config", &saccade_core::paths::portable(&auto))?,
                    )?
                } else {
                    RunConfig::default()
                }
            }
        };
        apply_run_args(args, &mut cfg)?;
        self.apply_perf_args(args, &mut cfg.perf)?;
        let images = arg_bool(args, "include_images")?.unwrap_or(false);
        self.run_and_explain((&baseline, &capture, &out), &cfg, images)
    }

    #[cfg(feature = "graphics")]
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
        self.validate_config(&cfg)?;
        Ok(cfg)
    }

    #[cfg(feature = "graphics")]
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
        let baseline = self.resolve("baseline_dir", &require_str(args, "baseline_dir")?)?;
        let capture = self.resolve("capture_dir", &require_str(args, "capture_dir")?)?;
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
        self.output_paths(&mut structured, cfg.record_absolute_paths);
        round_floats(&mut structured);
        Ok(ToolOutput {
            structured,
            text: report.text(),
            images: Vec::new(),
        })
    }

    #[cfg(feature = "graphics")]
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
        self.output_paths(&mut structured, cfg.record_absolute_paths);
        round_floats(&mut structured);
        Ok(ToolOutput {
            structured,
            text: report.text(),
            images: Vec::new(),
        })
    }

    fn tool_identity(&self, args: &Map<String, Value>) -> ToolResult {
        if args.contains_key("threshold") || args.contains_key("metric") {
            return Err(CliError::usage(
                "identity rejects threshold and metric; use saccade_compare for perceptual thresholds",
            ));
        }
        let mut known = vec!["parent_dir", "candidate_dir"];
        known.extend_from_slice(RUN_ARGS);
        reject_unknown(args, &known)?;
        let parent = self.resolve("parent_dir", &require_str(args, "parent_dir")?)?;
        let candidate = self.resolve("candidate_dir", &require_str(args, "candidate_dir")?)?;
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
        self.apply_perf_args(args, &mut cfg.perf)?;
        let images = arg_bool(args, "include_images")?.unwrap_or(false);
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
                "entries",
                "stretch",
                "seed",
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
            top: arg_top(args)?.min(5),
            stretch: arg_bool(args, "stretch")?.unwrap_or(false),
            entries: arg_strings(args, "entries")?,
            seed: args
                .get("seed")
                .map(|v| {
                    v.as_u64()
                        .ok_or_else(|| CliError::usage("seed must be an integer"))
                })
                .transpose()?,
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
        self.document_inputs(&report_json)?;
        let pack = explain(&report_json, &out, &opts)?;
        let mut value = crate::local_cmd::base_result("evidence");
        value["artifact"] =
            crate::local_cmd::reference(&out.join(saccade_core::explain::EXPLAIN_FILE))?;
        value["limits"] = json!([
            "Independent blind review must exclude the private key and implementation context."
        ]);
        let text = "Prepared local evidence.".into();
        let images = if arg_bool(args, "include_images")?.unwrap_or(false) {
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

    fn call_tool(&self, name: &str, args: &Map<String, Value>) -> Option<ToolResult> {
        if !matches!(
            name,
            // wave6
            "saccade_general"
                | "saccade_measure"
                | "saccade_inspect"
                | "saccade_evidence"
                | "saccade_propose"
                | "saccade_ask_human"
                | "saccade_review"
        ) || (name == "saccade_review" && !cfg!(feature = "ai"))
        {
            return None;
        }
        #[cfg(feature = "ai")]
        if name == "saccade_review" {
            return Some(self.provider_review(args));
        }
        Some(self.local_tool(name, args))
    }
    #[cfg(feature = "ai")]
    fn provider_review(&self, args: &Map<String, Value>) -> ToolResult {
        reject_unknown(
            args,
            &[
                "operation",
                "artifact",
                "out",
                "budget_calls",
                "expected_case_id",
            ],
        )?;
        let operation = require_str(args, "operation")?;
        if !["preview", "run"].contains(&operation.as_str()) {
            return Err(CliError::usage("review operation must be preview or run"));
        }
        let run = operation == "run";
        let config = crate::review_cmd::user_file(self.providers.user_config.as_deref());
        let user = crate::review_cmd::load_user(&config)?;
        let startup_budget = self.providers.budget_calls.unwrap_or(0);
        let auth = crate::review_cmd::authorization(
            self.providers.allow_provider_calls,
            startup_budget,
            &self.run_id,
            &user,
        );
        if run {
            auth.check()
                .map_err(|e| CliError::new("network_authorization_required", e))?;
        }
        let budget = match args.get("budget_calls") {
            None => startup_budget,
            Some(v) => v
                .as_u64()
                .filter(|n| *n > 0 && *n <= startup_budget)
                .ok_or_else(|| {
                    CliError::usage(
                        "tool budget must be positive and cannot exceed startup authorization",
                    )
                })?,
        };
        let file = self.existing_file("artifact", &require_str(args, "artifact")?)?;
        self.document_inputs(&file)?;
        if let Some(reference) = args.get("artifact").and_then(Value::as_object) {
            let actual = format!("sha256:{}", saccade_core::run::sha256_file(&file)?);
            if reference.get("sha256").and_then(Value::as_str) != Some(actual.as_str()) {
                return Err(CliError::new("stale_action", "artifact digest changed"));
            }
        }
        if let Some(expected) = arg_str(args, "expected_case_id")? {
            let value = crate::local_cmd::read_value(&file)?;
            let actual = value["case_id"].as_str().map(str::to_owned).or_else(|| {
                crate::read_report(&file)
                    .ok()
                    .and_then(|r| crate::local_cmd::case_for_result(&r, &file).ok())
                    .map(|c| c.case_id.as_str().to_owned())
            });
            if actual.as_deref() != Some(expected.as_str()) {
                return Err(CliError::new(
                    "stale_action",
                    "review case identity changed",
                ));
            }
        }
        let out = arg_str(args, "out")?
            .map(|p| self.resolve("out", &p))
            .transpose()?;
        if run && out.is_none() {
            return Err(CliError::usage("review run requires an out artifact"));
        }
        let value = crate::review_cmd::review(
            &file,
            out.as_deref(),
            run,
            budget,
            &config,
            &self.policy,
            &auth,
            None,
        )?;
        Ok(ToolOutput {
            structured: value,
            text: if run {
                "Recorded budgeted review proposals; human review remains unresolved."
            } else {
                "Prepared a local review plan; zero provider dispatches."
            }
            .into(),
            images: vec![],
        })
    }
    // wave6
    fn wave6_tool(&self, args: &Map<String, Value>) -> ToolResult {
        use clap::ValueEnum;
        reject_unknown(
            args,
            &[
                "operation",
                "reference",
                "capture",
                "out",
                "align",
                "resample",
                "threshold",
                "metric",
            ],
        )?;
        if require_str(args, "operation")? != "registered_compare" {
            return Err(CliError::usage("unknown general operation"));
        }
        let resolve_input = |key: &str| -> Result<PathBuf, CliError> {
            let path = self.resolve(key, &require_str(args, key)?)?;
            if path.is_dir() {
                self.input_tree(&path)?;
            } else if !path.is_file() {
                return Err(CliError::io("input must be file or directory"));
            }
            Ok(path)
        };
        let reference = resolve_input("reference")?;
        let capture = resolve_input("capture")?;
        let out = self.checked_out_dir(&require_str(args, "out")?, &[&reference, &capture])?;
        let align = crate::general_cmd::Align::from_str(&require_str(args, "align")?, false)
            .map_err(CliError::usage)?;
        let resample = arg_str(args, "resample")?
            .map(|v| crate::general_cmd::Resample::from_str(&v, false).map_err(CliError::usage))
            .transpose()?;
        let metric = match arg_str(args, "metric")?.as_deref().unwrap_or("mean") {
            "mean" => crate::MetricArg::Mean,
            "p95" => crate::MetricArg::P95,
            "p99" => crate::MetricArg::P99,
            "max" => crate::MetricArg::Max,
            _ => return Err(CliError::usage("invalid metric")),
        };
        let value = crate::general_cmd::compare_document(
            &reference,
            &capture,
            &out,
            &crate::general_cmd::CompareArgs {
                align: Some(align),
                resample,
            },
            arg_f64(args, "threshold")?.unwrap_or(0.02),
            metric,
        )?;
        let file = crate::general_cmd::persist_document(&value, &out)?;
        Ok(ToolOutput{structured:json!({"schema":saccade_core::general::RESULT_SCHEMA,"mode":"registered_compare","verdict":value["verdict"],"data":{"schema":value["schema"],"counts":value["counts"]},"artifacts":[{"path":saccade_core::paths::record(&file,&self.root,false)}],"next_actions":[]}),text:"Registered comparison over explicit geometric overlap; inspect model, residual and exclusion mask.".into(),images:Vec::new()})
    }

    fn local_tool(&self, name: &str, args: &Map<String, Value>) -> ToolResult {
        // wave6
        if name == "saccade_general" {
            return self.wave6_tool(args);
        }
        let operation = require_str(args, "operation")?;
        if let Some(case) = arg_str(args, "expected_case_id")? {
            let artifact = self.existing_file("artifact", &require_str(args, "artifact")?)?;
            let value = crate::local_cmd::read_value(&artifact)?;
            self.document_inputs(&artifact)?;
            if artifact.with_file_name("evidence.json").is_file() {
                self.document_inputs(&artifact.with_file_name("evidence.json"))?;
            }
            let current = if let Some(case) = value["case_id"].as_str() {
                case.to_owned()
            } else {
                crate::local_cmd::case_for_result(&crate::read_report(&artifact)?, &artifact)?
                    .case_id
                    .as_str()
                    .to_owned()
            };
            if current != case {
                return Err(CliError::new(
                    "stale_action",
                    "expected_case_id does not match current evidence",
                ));
            }
        }
        if let Some(reference) = args.get("artifact").filter(|v| v.is_object()) {
            let path = self.existing_file("artifact", &require_str(args, "artifact")?)?;
            let actual = format!("sha256:{}", saccade_core::run::sha256_file(&path)?);
            if reference["sha256"].as_str() != Some(actual.as_str()) {
                return Err(CliError::new("stale_action", "artifact digest changed"));
            }
        }
        let mut mapped = args.clone();
        mapped.remove("operation");
        mapped.remove("expected_case_id");
        if let Some(out) = mapped.remove("out") {
            mapped.insert("out_dir".into(), out);
        }
        if let Some(entry) = mapped.remove("entry") {
            mapped.insert(
                "entries".into(),
                if entry.is_array() {
                    entry
                } else {
                    json!([entry])
                },
            );
        }
        let result = match (name, operation.as_str()) {
            ("saccade_measure", "noise") => {
                reject_unknown(
                    args,
                    &[
                        "operation",
                        "dirs",
                        "out",
                        "kind",
                        "margin",
                        "metric",
                        "perf_name",
                        "perf_noise",
                        "perf_noise_k",
                        "perf_resolution_ms",
                        "perf_resolution_ticks",
                        "perf_min_delta_ms",
                        "perf_min_delta_pct",
                    ],
                )?;
                let dirs = arg_strings(args, "dirs")?
                    .iter()
                    .map(|p| self.existing_dir("dirs", p))
                    .collect::<Result<Vec<_>, _>>()?;
                let out = self.resolve("out", &require_str(args, "out")?)?;
                let mut perf = saccade_core::perf::PerfOptions::default();
                self.apply_perf_args(args, &mut perf)?;
                let metric = match arg_str(args, "metric")?.as_deref().unwrap_or("p95") {
                    "mean" => Metric::Mean,
                    "p95" => Metric::P95,
                    "p99" => Metric::P99,
                    "max" => Metric::Max,
                    _ => return Err(CliError::usage("invalid noise metric")),
                };
                let value = crate::local_cmd::measure_noise(
                    &dirs,
                    &out,
                    arg_str(args, "kind")?.as_deref().unwrap_or("image"),
                    arg_f64(args, "margin")?.unwrap_or(1.5),
                    metric,
                    &perf,
                )?;
                ToolOutput {
                    structured: value,
                    text: "Measured local repeat variation.".into(),
                    images: Vec::new(),
                }
            }
            #[cfg(feature = "graphics")]
            ("saccade_measure", "bisect") => {
                reject_unknown(
                    args,
                    &[
                        "operation",
                        "runs",
                        "good",
                        "out",
                        "threshold",
                        "metric",
                        "entry",
                    ],
                )?;
                let runs = arg_strings(args, "runs")?
                    .iter()
                    .map(|p| self.existing_dir("runs", p))
                    .collect::<Result<Vec<_>, _>>()?;
                let good = arg_str(args, "good")?
                    .map(|p| self.existing_dir("good", &p))
                    .transpose()?;
                let out = self.resolve("out", &require_str(args, "out")?)?;
                let options = crate::s6::options(
                    arg_f64(args, "threshold")?,
                    arg_str(args, "metric")?.as_deref(),
                    arg_str(args, "entry")?,
                )?;
                let report = saccade_core::bisect::runs(&runs, good.as_deref(), &out, &options)?;
                ToolOutput {
                    structured: crate::local_cmd::analysis_result(
                        &serde_json::to_value(&report)?,
                        &out,
                    )?,
                    text: "Analyzed ordered existing captures.".into(),
                    images: Vec::new(),
                }
            }
            #[cfg(feature = "prechecks")]
            ("saccade_measure", op @ ("safety" | "a11y")) => {
                let name = format!("saccade_{op}");
                let (structured, text) =
                    crate::precheck_mcp::call(&name, &mapped, &|key, path| self.resolve(key, path))
                        .ok_or_else(|| CliError::usage("unavailable precheck"))??;
                ToolOutput {
                    structured,
                    text,
                    images: Vec::new(),
                }
            }
            ("saccade_inspect", "config") => {
                reject_unknown(args, &["operation", "config", "entry"])?;
                let file = arg_str(args, "config")?
                    .map(|p| self.existing_file("config", &p))
                    .transpose()?;
                let cfg = match &file {
                    Some(p) => RunConfig::from_toml_file(p)?,
                    None => RunConfig::default(),
                };
                self.validate_config(&cfg)?;
                let mut value = crate::local_cmd::base_result("config");
                value["data"] = cfg.explain_settings(
                    file.as_deref(),
                    if file.is_some() {
                        "explicit config"
                    } else {
                        "built-in defaults"
                    },
                    arg_str(args, "entry")?.as_deref(),
                )?;
                if let Some(data) = value["data"].as_object_mut() {
                    data.remove("defaults");
                }
                ToolOutput {
                    structured: crate::local_cmd::bounded(value, 4096)?,
                    text: "Inspected local effective settings.".into(),
                    images: Vec::new(),
                }
            }
            ("saccade_measure", "compare") => self.tool_compare(&mapped)?,
            ("saccade_measure", "identity") => self.tool_identity(&mapped)?,
            #[cfg(feature = "graphics")]
            ("saccade_measure", "ablate") => self.tool_ablate(&mapped)?,
            #[cfg(feature = "graphics")]
            ("saccade_measure", "sequence") => self.tool_sequence(&mapped)?,
            #[cfg(feature = "graphics")]
            ("saccade_measure", "rank") => self.tool_rank(&mapped)?,
            ("saccade_inspect", "capabilities") => {
                reject_unknown(args, &["operation"])?;
                let mut value = crate::local_cmd::base_result("capabilities");
                value["limits"] = json!([
                    "Provider review requires explicit startup authorization, a budget and allowed provenance."
                ]);
                value["counts"] = json!({"tools":tool_schemas().as_array().map_or(0,Vec::len)});
                value["data"] = json!({"features":saccade_core::COMPILED_FEATURES,"tools":tool_schemas().as_array().map(|a|a.iter().map(|t|t["name"].clone()).collect::<Vec<_>>()),"contracts":["saccade-result.v2","saccade-evidence.v1","saccade-report.v1"]});
                return Ok(ToolOutput {
                    structured: value,
                    text: "Compiled tools and provider authorization policy.".into(),
                    images: Vec::new(),
                });
            }
            ("saccade_inspect", "grounded") => {
                reject_unknown(
                    args,
                    &[
                        "operation",
                        "artifact",
                        "limit",
                        "cursor",
                        "expected_case_id",
                    ],
                )?;
                let path = self.existing_file("artifact", &require_str(args, "artifact")?)?;
                self.document_inputs(&path)?;
                let limit = args
                    .get("limit")
                    .map(|v| {
                        v.as_u64()
                            .filter(|n| *n <= 5)
                            .ok_or_else(|| CliError::usage("grounded limit must be 1..5"))
                    })
                    .transpose()?
                    .unwrap_or(3) as usize;
                let value =
                    crate::grounded_cmd::page(&path, limit, arg_str(args, "cursor")?.as_deref())?;
                return Ok(ToolOutput {structured:value,text:"Verified numerical observations with source, region and evidence citations; semantic and causal claims unproven.".into(),images:Vec::new()});
            }
            ("saccade_inspect", "summary" | "entries" | "request_status") => {
                reject_unknown(
                    args,
                    &[
                        "operation",
                        "artifact",
                        "entry",
                        "status",
                        "limit",
                        "cursor",
                        "expected_case_id",
                    ],
                )?;
                let path = self.existing_file("artifact", &require_str(args, "artifact")?)?;
                self.document_inputs(&path)?;
                let limit = args
                    .get("limit")
                    .map(|v| {
                        v.as_u64()
                            .and_then(|n| usize::try_from(n).ok())
                            .ok_or_else(|| CliError::usage("limit must be an integer"))
                    })
                    .transpose()?
                    .unwrap_or(if operation == "entries" { 10 } else { 5 });
                if operation != "entries" && limit > 5 {
                    return Err(CliError::usage(
                        "summaries list at most five entries; use the entries operation for pages",
                    ));
                }
                let value = crate::local_cmd::inspect_page(
                    &path,
                    arg_str(args, "entry")?.as_deref(),
                    false,
                    &arg_strings(args, "status")?,
                    limit,
                    arg_str(args, "cursor")?.as_deref(),
                )?;
                return Ok(ToolOutput {
                    text: format!("Inspected evidence; {} omitted.", value["page"]["omitted"]),
                    structured: value,
                    images: Vec::new(),
                });
            }
            ("saccade_evidence", "context" | "crops") => {
                mapped.remove("artifact");
                mapped.insert("report_json".into(), json!(require_str(args, "artifact")?));
                self.tool_explain(&mapped)?
            }
            ("saccade_evidence", "request") => {
                reject_unknown(
                    args,
                    &[
                        "operation",
                        "artifact",
                        "question",
                        "out",
                        "expected_case_id",
                    ],
                )?;
                let path = self.existing_file("artifact", &require_str(args, "artifact")?)?;
                self.document_inputs(&path)?;
                let out = self.resolve("out", &require_str(args, "out")?)?;
                let value =
                    crate::local_cmd::request(&path, &require_str(args, "question")?, &out)?;
                return Ok(ToolOutput {
                    structured: value,
                    text: "Prepared existing closed request locally.".into(),
                    images: Vec::new(),
                });
            }
            ("saccade_evidence", "snapshot") => {
                reject_unknown(
                    args,
                    &[
                        "operation",
                        "artifact",
                        "entry",
                        "out",
                        "state",
                        "width",
                        "include_images",
                        "expected_case_id",
                    ],
                )?;
                let path = self.existing_file("artifact", &require_str(args, "artifact")?)?;
                self.document_inputs(&path)?;
                let out = self.resolve("out", &require_str(args, "out")?)?;
                let width = args
                    .get("width")
                    .map(|v| {
                        v.as_u64()
                            .and_then(|v| u32::try_from(v).ok())
                            .filter(|v| *v > 0 && *v <= 4096)
                            .ok_or_else(|| CliError::usage("width must be 1..4096"))
                    })
                    .transpose()?
                    .unwrap_or(1024);
                let snap = crate::agent_ui::render_snapshot(
                    &path,
                    Some(&require_str(args, "entry")?),
                    arg_str(args, "state")?.as_deref(),
                    width,
                    &out,
                )?;
                let mut value = crate::local_cmd::base_result("snapshot");
                value["artifact"] = crate::local_cmd::reference(
                    snap.paths
                        .first()
                        .ok_or_else(|| CliError::io("snapshot produced no frame"))?,
                )?;
                let images = if arg_bool(args, "include_images")?.unwrap_or(false) {
                    snap.paths
                        .iter()
                        .take(MAX_IMAGES)
                        .filter_map(|p| image_block(p))
                        .collect()
                } else {
                    Vec::new()
                };
                return Ok(ToolOutput {
                    structured: value,
                    text: format!("Snapshot: {} × {}.", snap.width, snap.height),
                    images,
                });
            }
            ("saccade_propose", "answers") => {
                reject_unknown(
                    args,
                    &[
                        "operation",
                        "artifact",
                        "answers",
                        "out",
                        "expected_case_id",
                    ],
                )?;
                let request = self.existing_file("artifact", &require_str(args, "artifact")?)?;
                self.document_inputs(&request)?;
                let answers = self.existing_file("answers", &require_str(args, "answers")?)?;
                self.document_inputs(&answers)?;
                let out = self.resolve("out", &require_str(args, "out")?)?;
                let value = crate::local_cmd::propose(&request, &answers, Some(&out))?;
                return Ok(ToolOutput {
                    structured: value,
                    text: "Recorded a proposal; human review remains unresolved.".into(),
                    images: Vec::new(),
                });
            }
            ("saccade_ask_human", "request") => {
                reject_unknown(args, &["operation", "artifact", "out", "expected_case_id"])?;
                let request = self.existing_file("artifact", &require_str(args, "artifact")?)?;
                self.document_inputs(&request)?;
                let out = self.resolve("out", &require_str(args, "out")?)?;
                let value = crate::local_cmd::ask(&request, Some(&out))?;
                return Ok(ToolOutput {
                    structured: value,
                    text: "Human review item is unresolved.".into(),
                    images: Vec::new(),
                });
            }
            ("saccade_measure", op) if crate::unavailable_feature(op).is_some() => {
                return Err(saccade_core::Error::FeatureUnavailable {
                    feature: crate::unavailable_feature(op).unwrap_or("graphics"),
                }
                .into());
            }
            _ => {
                return Err(CliError::usage(
                    "operation is not implemented for this local tool",
                ));
            }
        };
        let mut result = result;
        if result.structured["schema"] != "saccade-result.v2" {
            let mut value = crate::local_cmd::base_result(&operation);
            if let Some(out) = args.get("out").and_then(Value::as_str) {
                let out = self.resolve("out", out)?;
                let file = out.join(format!(
                    "{}.json",
                    result.structured["schema"].as_str().unwrap_or("explain")
                ));
                if file.is_file() {
                    value["artifact"] = crate::local_cmd::reference(&file)?;
                } else if out.join("explain.json").is_file() {
                    value["artifact"] = crate::local_cmd::reference(&out.join("explain.json"))?;
                }
            }
            result.structured = value;
        }
        result.text = format!(
            "{} completed; measurement {}.",
            operation,
            result.structured["measurement"]
                .as_str()
                .unwrap_or("unknown")
        );
        Ok(result)
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
        // No id: a notification such as notifications/initialized, never answered.
        let id = id?;
        let params = obj.get("params").and_then(Value::as_object);
        let result = match method {
            "initialize" => json!({
                "protocolVersion": PROTOCOL_VERSION,
                "capabilities": {"tools": {"listChanged": false}, "logging": {}},
                "serverInfo": {"name": "saccade", "version": env!("CARGO_PKG_VERSION")},
                "instructions": "Measure locally with saccade_measure, inspect bounded results, then prepare evidence or ask a human. Capture roots are read-only; generated artifacts require --out-root. Provider calls require explicit human startup authorization, a finite budget and permitted source provenance. Images are returned only when requested.",
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
                    Some(_) => {
                        return Some(rpc_error(id, -32602, "`arguments` must be an object"));
                    }
                };
                match self.call_tool(name, args) {
                    Some(r) => tool_response(r.map(|mut out| {
                        self.output_paths(&mut out.structured, false);
                        out
                    })),
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
            "content": [{"type": "text", "text": format!("saccade error [{}]: {}", e.code, crate::local_cmd::short(&e.message, 256))}],
            "structuredContent": e.value(),
            "isError": true,
        }),
    }
}

fn rpc_error(id: Value, code: i64, message: &str) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
}

/// Serves the explicit startup registry on stdin/stdout until stdin closes.
pub fn serve_stdio(
    roots: &[PathBuf],
    output: Option<&Path>,
    follow: bool,
    targets: &[PathBuf],
    #[cfg(feature = "ai")] providers: crate::review_cmd::Startup,
) -> Result<(), CliError> {
    #[allow(unused_mut)]
    let mut server = Server::new(roots, output, follow, targets)?;
    #[cfg(feature = "ai")]
    {
        if providers.allow_provider_calls != providers.budget_calls.is_some() {
            return Err(CliError::usage(
                "MCP provider calls require both --allow-provider-calls and a positive --budget-calls",
            ));
        }
        server.providers = providers;
    }
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

fn tool_schemas() -> Value {
    let historical = measurement_schemas();
    let mut measures = Vec::new();
    for (name, operation) in [
        ("compare", "compare"),
        ("identity", "identity"),
        ("ablate", "ablate"),
        ("sequence", "sequence"),
        ("rank", "rank"),
    ] {
        if crate::unavailable_feature(operation).is_some() {
            continue;
        }
        if let Some(old) = historical
            .as_array()
            .and_then(|a| a.iter().find(|t| t["operation"] == name))
        {
            let mut schema = old["inputSchema"].clone();
            if let Some(props) = schema["properties"].as_object_mut() {
                props.insert(
                    "operation".into(),
                    json!({"const":operation,"type":"string"}),
                );
                if let Some(mut out) = props.remove("out_dir") {
                    out["description"] =
                        json!("Generated artifact path inside the separately authorized out-root.");
                    props.insert("out".into(), out);
                }
                if let Some(entry) = props.remove("entries") {
                    props.insert("entry".into(), entry);
                }
                if let Some(include) = props.get_mut("include_images") {
                    include["default"] = json!(false);
                }
                #[cfg(not(feature = "graphics"))]
                props.retain(|k, _| !k.starts_with("perf_"));
            }
            if let Some(required) = schema["required"].as_array_mut() {
                for r in required.iter_mut() {
                    if *r == "out_dir" {
                        *r = json!("out");
                    }
                }
                required.push(json!("operation"));
            }
            measures.push(schema);
        }
    }
    measures.push(json!({"type":"object","properties":{"operation":{"const":"noise","type":"string"},"dirs":{"type":"array","items":{"type":"string"},"minItems":2},"out":{"type":"string"},"kind":{"type":"string","enum":if cfg!(feature="graphics"){vec!["image","performance"]}else{vec!["image"]}},"margin":{"type":"number","minimum":1},"metric":{"enum":["mean","p95","p99","max"]},"perf_name":{"type":"string"},"perf_noise":{"type":"string"},"perf_noise_k":{"type":"number","exclusiveMinimum":0},"perf_resolution_ms":{"type":"number","exclusiveMinimum":0},"perf_resolution_ticks":{"type":"integer","minimum":1},"perf_min_delta_ms":{"type":"number","minimum":0},"perf_min_delta_pct":{"type":"number","minimum":0}},"required":["operation","dirs","out"],"additionalProperties":false}));
    #[cfg(feature="graphics")]
    measures.push(json!({"type":"object","properties":{"operation":{"const":"bisect","type":"string"},"runs":{"type":"array","items":{"type":"string"},"minItems":2},"good":{"type":"string"},"out":{"type":"string"},"threshold":{"type":"number","minimum":0,"maximum":1},"metric":{"enum":["mean","p95","p99","max"]},"entry":{"type":"string"}},"required":["operation","runs","out"],"additionalProperties":false}));
    #[cfg(feature = "prechecks")]
    for old in crate::precheck_mcp::schemas() {
        let mut schema = old["inputSchema"].clone();
        if let Some(props) = schema["properties"].as_object_mut() {
            props.insert("operation".into(),json!({"const":if old["name"]=="saccade_safety"{"safety"}else{"a11y"},"type":"string"}));
            if let Some(out) = props.remove("out_dir") {
                props.insert("out".into(), out);
            }
        }
        if let Some(required) = schema["required"].as_array_mut() {
            for r in required.iter_mut() {
                if *r == "out_dir" {
                    *r = json!("out");
                }
            }
            required.push(json!("operation"));
        }
        measures.push(schema);
    }
    #[cfg(not(feature = "graphics"))]
    for schema in &mut measures {
        if let Some(props) = schema["properties"].as_object_mut() {
            props.retain(|key, _| !key.starts_with("perf_"));
        }
    }
    let common = json!({"artifact":{"oneOf":[{"type":"string"},{"type":"object","properties":{"path":{"type":"string"},"sha256":{"type":"string","pattern":"^sha256:[0-9a-f]{64}$"}},"required":["path","sha256"],"additionalProperties":false}]},"out":{"type":"string"},"entry":{"type":"string"},"include_images":{"type":"boolean","default":false},"expected_case_id":{"type":"string","pattern":"^sha256:[0-9a-f]{64}$"}});
    let make = |name: &str, description: &str, operations: Vec<(&str, Vec<&str>, Value)>| {
        let variants=operations.into_iter().map(|(op,required,extra)|{
            let mut properties=common.as_object().cloned().unwrap_or_default();
            properties.retain(|key,_|match key.as_str(){
                "artifact"=>op!="capabilities" && op!="config",
                "out"=>name!="saccade_inspect",
                "entry"=>matches!(op,"summary"|"entries"|"context"|"crops"|"snapshot"|"config"),
                "include_images"=>name=="saccade_evidence" && matches!(op,"context"|"crops"|"snapshot"),
                "expected_case_id"=>op!="capabilities" && op!="config",_=>false});
            properties.insert("operation".into(),json!({"type":"string","const":op}));
            if let Some(extra)=extra.as_object(){properties.extend(extra.clone());}
            let mut fields=vec!["operation"];fields.extend(required);
            json!({"type":"object","properties":properties,"required":fields,"additionalProperties":false})
        }).collect::<Vec<_>>();
        json!({"name":name,"description":description,"inputSchema":{"type":"object","oneOf":variants},"outputSchema":{"type":"object","properties":{"schema":{"const":"saccade-result.v2"}},"required":["schema"]},"annotations":{"destructiveHint":false,"openWorldHint":false}})
    };
    #[allow(unused_mut)]
    let mut schemas = json!([
        {"name":"saccade_measure","description":"Local measurements; regressions remain normal results. Inputs are read-only; outputs require out-root. No images by default.","inputSchema":{"type":"object","oneOf":measures},"outputSchema":{"type":"object"},"annotations":{"destructiveHint":false,"openWorldHint":false}},
        make("saccade_inspect","Read bounded evidence pages and local capabilities.",vec![
            ("summary",vec!["artifact"],json!({"limit":{"type":"integer","minimum":1,"maximum":10},"status":{"type":"array","items":{"type":"string"}},"cursor":{"type":"string"}})),
            ("entries",vec!["artifact"],json!({"limit":{"type":"integer","minimum":1,"maximum":10},"status":{"type":"array","items":{"type":"string"}},"cursor":{"type":"string"}})),
            ("grounded",vec!["artifact"],json!({"limit":{"type":"integer","minimum":1,"maximum":5},"cursor":{"type":"string"}})),
            ("request_status",vec!["artifact"],json!({})),("capabilities",vec![],json!({})),("config",vec![],json!({"config":{"type":"string"}}))]),
        make("saccade_evidence","Prepare local context/crops, an existing closed request, or a selected snapshot.",vec![
            ("context",vec!["artifact","out"],json!({"top":{"type":"integer","minimum":0,"maximum":5},"stretch":{"type":"boolean"},"blind":{"type":"boolean"},"key_out":{"type":"string"},"seed":{"type":"integer"}})),
            ("crops",vec!["artifact","out"],json!({"top":{"type":"integer","minimum":0,"maximum":5},"stretch":{"type":"boolean"}})),
            ("request",vec!["artifact","question","out"],json!({"question":{"type":"string"}})),
            ("snapshot",vec!["artifact","entry","out"],json!({"state":{"type":"string"},"width":{"type":"integer","minimum":1,"maximum":4096}}))]),
        make("saccade_propose","Validate and record a proposal against an existing closed request; never approval.",vec![("answers",vec!["artifact","answers","out"],json!({"answers":{"type":"string"}}))]),
        make("saccade_ask_human","Create or retrieve an unresolved closed-request item locally.",vec![("request",vec!["artifact","out"],json!({}))])
    ]);
    #[cfg(feature = "ai")]
    if let Some(list) = schemas.as_array_mut() {
        let mut tool = make(
            "saccade_review",
            "Preview or execute review under human startup authority and shared attempt budgets; models propose only.",
            vec![
                (
                    "preview",
                    vec!["artifact"],
                    json!({"budget_calls":{"type":"integer","minimum":1}}),
                ),
                (
                    "run",
                    vec!["artifact", "out"],
                    json!({"budget_calls":{"type":"integer","minimum":1}}),
                ),
            ],
        );
        tool["annotations"]["openWorldHint"] = json!(true);
        list.push(tool);
    }
    // wave6
    if let Some(list) = schemas.as_array_mut() {
        list.push(crate::general_cmd::tool_schema());
    }
    schemas
}
