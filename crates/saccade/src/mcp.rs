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

#[cfg(feature = "products")]
#[path = "product_mcp.rs"]
mod products;

/// The server: the root every path must stay under.
pub struct Server {
    // Authority comes from server startup, never tool arguments.
    #[cfg(feature = "products")]
    product_network: bool,
    #[cfg(feature = "products")]
    product_notifications: bool,
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
            "intended_variables":{"type":"array","items":{"type":"string"}},
            "require_valid_arms":{"type":"boolean"},
            "fingerprint_map":{"type":"string"},
            "arm_ignore":{"type":"array","items":{"type":"string"}},
            "allow_unreached":{"type":"array","items":{"type":"string"}},
            "export_maps":{"type":"boolean"},"require_scope":{"type":"boolean"},"render_evidence":{"type":"boolean"},
            "noise_from":{"type":"array","minItems":2,"maxItems":32,"items":{"type":"string"}},"mask_dump":{"type":"string"},"mask_from_dump":{"type":"string"},"mask_layer":{"type":"string"},"require_effect":{"type":"array","items":{"type":"string"}},"id_top":{"type":"integer","minimum":0,"maximum":32},"id_threshold":{"type":"number","minimum":0},
            "fixed_camera":{"type":"boolean"},
            "fail_on_new":{"type":"boolean","default":true},
            "allow_empty":{"type":"boolean","default":false},
            "include_images":{"type":"boolean","default":false},
            "entries":{"type":"array","items":{"type":"string"}},
            "record_absolute_paths":{"type":"boolean","default":false},
            "gpu_clock_map":{"type":"string"},"perf_name":{"type":"string"},"perf_noise":{"type":"string"},
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
                "require_valid_arms":{"type":"boolean"}, "fingerprint_map":{"type":"string"},
                "intended_variables":{"type":"array","items":{"type":"string"}},
                "arm_ignore":{"type":"array","items":{"type":"string"}},
            "allow_unreached":{"type":"array","items":{"type":"string"}},
            "export_maps":{"type":"boolean"},"require_scope":{"type":"boolean"},"render_evidence":{"type":"boolean"},
            "noise_from":{"type":"array","minItems":2,"maxItems":32,"items":{"type":"string"}},"mask_dump":{"type":"string"},"mask_from_dump":{"type":"string"},"mask_layer":{"type":"string"},"require_effect":{"type":"array","items":{"type":"string"}},"id_top":{"type":"integer","minimum":0,"maximum":32},"id_threshold":{"type":"number","minimum":0},
                "top":{"type":"integer","minimum":0,"default":5},
                "gpu_clock_map":{"type":"string"},"perf_name":{"type":"string"}, "perf_noise":{"type":"string"},
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

/// Operator-resolved embedding inputs (see `operator_model`); the guard keeps a
/// materialised registry contract alive for the duration of the call.
#[derive(Default)]
struct OperatorModel {
    guard: Option<tempfile::TempDir>,
    model: Option<PathBuf>,
    cache: Option<PathBuf>,
    library: Option<PathBuf>,
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
    "gpu_clock_map",
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
    "intended_variables",
    "require_valid_arms",
    "fingerprint_map",
    "arm_ignore",
    "allow_unreached",
    "export_maps",
    "require_scope",
    "render_evidence",
    "noise_from",
    "mask_dump",
    "mask_from_dump",
    "mask_layer",
    "require_effect",
    "id_top",
    "id_threshold",
    "fixed_camera",
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
    if arg_bool(args, "fixed_camera")?.unwrap_or(false) && cfg.temporal_tiles.is_none() {
        cfg.temporal_tiles = Some(Default::default());
    }
    cfg.meta
        .intended
        .extend(arg_strings(args, "intended_variables")?);
    cfg.meta.require_valid_arms |= arg_bool(args, "require_valid_arms")?.unwrap_or(false);
    if let Some(map) = arg_str(args, "fingerprint_map")? {
        cfg.meta.fingerprint_map = Some(map.into());
    }
    cfg.meta.ignore.extend(arg_strings(args, "arm_ignore")?);
    cfg.meta
        .allow_unreached
        .extend(arg_strings(args, "allow_unreached")?);
    let field = crate::wave10_cmd::CompareArgs {
        export_maps: arg_bool(args, "export_maps")?.unwrap_or(false),
        require_scope: arg_bool(args, "require_scope")?.unwrap_or(false),
        noise_from: arg_strings(args, "noise_from")?
            .into_iter()
            .map(PathBuf::from)
            .collect(),
        mask_dump: arg_str(args, "mask_dump")?.or(arg_str(args, "mask_from_dump")?),
        mask_layer: arg_str(args, "mask_layer")?,
        require_effect: arg_strings(args, "require_effect")?,
        id_top: args
            .get("id_top")
            .map(|v| {
                v.as_u64()
                    .map(|n| n as usize)
                    .ok_or_else(|| CliError::usage("id_top requires an unsigned integer"))
            })
            .transpose()?,
        id_threshold: args
            .get("id_threshold")
            .map(|v| {
                v.as_f64()
                    .ok_or_else(|| CliError::usage("id_threshold requires a number"))
            })
            .transpose()?,
    };
    field.apply(cfg)?;
    if arg_bool(args, "render_evidence")?.unwrap_or(false) && cfg.buffers.is_empty() {
        cfg.spatial.get_or_insert_with(Default::default).decide = true;
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
            #[cfg(feature = "products")]
            product_network: false,
            #[cfg(feature = "products")]
            product_notifications: false,
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
        if saccade_core::report_links::original_schema(value["schema"].as_str().unwrap_or_default())
            == saccade_core::report::REPORT_SCHEMA
        {
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
        // wave9: mask inclusion inputs obey the same registered read policy.
        for effect in &cfg.required_effect {
            if let saccade_core::evidence_quality::effect::Selection::Mask { image } =
                &effect.selection
            {
                self.policy
                    .read(&cfg.config_dir.as_deref().unwrap_or(&self.root).join(image))
                    .map_err(|e| CliError::new("unsafe_path", e.to_string()))?;
            }
        }
        for mask in &cfg.masks {
            if let Some(path) = &mask.image {
                self.policy
                    .read(&cfg.config_dir.as_deref().unwrap_or(&self.root).join(path))
                    .map_err(|e| CliError::new("unsafe_path", e.to_string()))?;
            }
        }
        if let Some(path) = &cfg.perf.gpu_clock_map {
            self.policy
                .read(path)
                .map_err(|e| CliError::new("unsafe_path", e.to_string()))?;
        }
        if let Some(path) = &cfg.perf.noise {
            self.policy
                .read(path)
                .map_err(|e| CliError::new("unsafe_path", e.to_string()))?;
        }
        for repeat in &cfg.field.noise_from {
            self.input_tree(&self.policy.read(repeat)?)?;
        }
        if let Some(path) = &cfg.meta.fingerprint_map {
            self.policy.read(path)?;
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

    /// Embedding model, cache and runtime for a request, from operator configuration only.
    /// `model` must be a contract id in the operator registry; a path is refused. `cache`
    /// and `library` may only restate the configured location.
    fn operator_model(
        &self,
        args: &Map<String, Value>,
        want: bool,
    ) -> Result<OperatorModel, CliError> {
        let refuse = |m: String| CliError::new("model_location_not_request_controlled", m);
        let cfg = crate::wave7_cmd::config()?;
        let mut out = OperatorModel::default();
        if !want
            && !["model", "cache", "library"]
                .iter()
                .any(|k| args.contains_key(*k))
        {
            return Ok(out);
        }
        if let Some(id) = arg_str(args, "model")? {
            let registry = crate::wave7_cmd::registry(None)?;
            let contract = registry
                .contracts
                .get(&id)
                .filter(|c| c["schema"] == saccade_core::general::embedding::MODEL_SCHEMA)
                .ok_or_else(|| {
                    refuse(
                        "`model` must be an embedding contract id in the operator registry; a request cannot supply a path"
                            .into(),
                    )
                })?;
            let dir = tempfile::tempdir().map_err(|e| CliError::io(e.to_string()))?;
            let file = dir.path().join("embedding-model.json");
            std::fs::write(&file, serde_json::to_vec(contract)?)
                .map_err(|e| CliError::io(e.to_string()))?;
            out.model = Some(file);
            out.guard = Some(dir);
        } else {
            out.model = cfg.embedding_contract.clone();
        }
        for (key, configured) in [
            ("cache", Some(cfg.dir.clone())),
            ("library", cfg.runtime_library.clone()),
        ] {
            if let Some(p) = arg_str(args, key)? {
                let requested = self.resolve(key, &p)?;
                cfg.check_request_location(key, &requested)
                    .map_err(|e| refuse(e.to_string()))?;
            }
            match key {
                "cache" => out.cache = configured,
                _ => out.library = configured,
            }
        }
        if let Some(c) = &out.cache
            && !c.is_dir()
        {
            return Err(CliError::io(
                "MCP embedding cache must already exist; downloads are explicit CLI-only (saccade models pull)",
            ));
        }
        Ok(out)
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
        for repeat in &cfg.field.noise_from {
            self.input_tree(&self.policy.read(repeat)?)?;
        }
        if let Some(path) = &cfg.meta.fingerprint_map {
            crate::arms_mcp::validate_map(&self.policy, path, &[baseline, capture])?;
        }
        let mut report: Report = crate::signed_approval::run(baseline, capture, out, cfg, None)?;
        // Retain the authorized alias route in references so downstream reads
        // do not turn a registered target into an independently browsable root.
        let base = out;
        report.baseline_dir = Some(crate::local_cmd::lexical_record(baseline, base));
        report.capture_dir = Some(crate::local_cmd::lexical_record(capture, base));
        report = serde_json::from_value(saccade_core::report_links::decorate(
            &serde_json::to_value(&report)?,
        )?)?;
        saccade_core::report_links::write(
            &out.join(saccade_core::report::REPORT_FILE_NAME),
            &report,
        )?;
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
        if let Some(n) = arg_str(args, "gpu_clock_map")? {
            opts.gpu_clock_map = Some(self.existing_file("gpu_clock_map", &n)?);
        }
        if let Some(n) = &opts.gpu_clock_map {
            opts.gpu_clock_map =
                Some(self.existing_file("gpu_clock_map", &saccade_core::paths::portable(n))?);
        }
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
                "base_repeats",
                "arm_repeats",
                "out_dir",
                "config",
                "top",
                "gpu_clock_map",
                "perf_name",
                "perf_noise",
                "perf_noise_k",
                "perf_resolution_ms",
                "perf_resolution_ticks",
                "perf_min_delta_ms",
                "perf_min_delta_pct",
                "record_absolute_paths",
                "require_valid_arms",
                "fingerprint_map",
                "intended_variables",
                "arm_ignore",
                "allow_unreached",
                "export_maps",
                "require_scope",
                "render_evidence",
                "noise_from",
                "mask_dump",
                "mask_from_dump",
                "mask_layer",
                "require_effect",
                "id_top",
                "id_threshold",
            ],
        )?;
        let base_names = arg_strings(args, "base_repeats")?;
        let bases = if base_names.is_empty() {
            vec![self.existing_dir("base_dir", &require_str(args, "base_dir")?)?]
        } else {
            base_names
                .iter()
                .map(|p| self.existing_dir("base_repeats", p))
                .collect::<Result<Vec<_>, _>>()?
        };
        let base = bases[0].clone();
        let arms = arg_strings(args, "arm_dirs")?
            .iter()
            .map(|p| self.existing_dir("arm_dirs", p))
            .collect::<Result<Vec<_>, _>>()?;
        let mut groups = Vec::new();
        if let Some(repeats) = args.get("arm_repeats") {
            let specs = repeats
                .as_array()
                .filter(|s| !s.is_empty() && s.len() <= 128)
                .ok_or_else(|| CliError::usage("arm_repeats needs 1..128 groups"))?;
            let mut labels = std::collections::BTreeSet::new();
            for spec in specs {
                let spec = spec
                    .as_object()
                    .ok_or_else(|| CliError::usage("repeat group must be an object"))?;
                reject_unknown(spec, &["label", "repeats"])?;
                let label = require_str(spec, "label")?;
                if label.is_empty() || !labels.insert(label.clone()) {
                    return Err(CliError::usage("repeat labels must be nonempty and unique"));
                }
                let names = arg_strings(spec, "repeats")?;
                if names.is_empty() || names.len() > 128 {
                    return Err(CliError::usage("group needs 1..128 repeats"));
                }
                let paths = names
                    .iter()
                    .map(|p| self.existing_dir("repeats", p))
                    .collect::<Result<Vec<_>, _>>()?;
                groups.push((label, paths));
            }
        } else {
            groups = arms
                .iter()
                .map(|p| (String::new(), vec![p.clone()]))
                .collect();
        }
        let out = self.resolve("out_dir", &require_str(args, "out_dir")?)?;
        let mut cfg = match arg_str(args, "config")? {
            Some(p) => RunConfig::from_toml_file(&self.existing_file("config", &p)?)?,
            None => RunConfig::default(),
        };
        apply_run_args(args, &mut cfg)?;
        cfg.field.noise_from = cfg
            .field
            .noise_from
            .iter()
            .map(|p| self.policy.read(p))
            .collect::<Result<Vec<_>, _>>()?;
        if let Some(map) = cfg.meta.fingerprint_map.as_mut() {
            *map = self.policy.read(map)?;
        }
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
        if let Some(map) = &cfg.meta.fingerprint_map {
            let inputs = std::iter::once(base.as_path())
                .chain(arms.iter().map(|p| p.as_path()))
                .collect::<Vec<_>>();
            crate::arms_mcp::validate_map(&self.policy, map, &inputs)?;
        }
        for p in bases.iter().chain(groups.iter().flat_map(|(_, p)| p)) {
            self.input_tree(p)?;
        }
        if let Some(map) = &cfg.meta.fingerprint_map {
            let inputs = bases
                .iter()
                .chain(groups.iter().flat_map(|(_, p)| p))
                .map(PathBuf::as_path)
                .collect::<Vec<_>>();
            crate::arms_mcp::validate_map(&self.policy, map, &inputs)?;
        }
        let model = saccade_core::ablate::run_repeats(&bases, &groups, &out, &cfg, top)?;
        let mut structured = serde_json::to_value(&model)?;
        crate::arms_cmd::annotate(&mut structured, &cfg.meta);
        Ok(ToolOutput {
            structured,
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
        cfg.field.noise_from = cfg
            .field
            .noise_from
            .iter()
            .map(|p| self.policy.read(p))
            .collect::<Result<Vec<_>, _>>()?;
        if let Some(map) = cfg.meta.fingerprint_map.as_mut() {
            *map = self.policy.read(map)?;
        }
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
        cfg.field.noise_from = cfg
            .field
            .noise_from
            .iter()
            .map(|p| self.policy.read(p))
            .collect::<Result<Vec<_>, _>>()?;
        if let Some(map) = cfg.meta.fingerprint_map.as_mut() {
            *map = self.policy.read(map)?;
        }
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
        cfg.field.noise_from = cfg
            .field
            .noise_from
            .iter()
            .map(|p| self.policy.read(p))
            .collect::<Result<Vec<_>, _>>()?;
        if let Some(map) = cfg.meta.fingerprint_map.as_mut() {
            *map = self.policy.read(map)?;
        }
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
        // wave11: every request has an isolated, output-root-contained report index.
        let refs = match arg_strings(args, "source_refs") {
            Ok(r) => r,
            Err(e) => return Some(Err(e)),
        };
        let _links = match saccade_core::report_links::scope(
            refs,
            self.policy
                .output_root()
                .map(|p| p.join("reports/index.jsonl")),
        ) {
            Ok(c) => c,
            Err(e) => return Some(Err(e.into())),
        };
        let mut clean_args = args.clone();
        if !args
            .get("operation")
            .and_then(Value::as_str)
            .is_some_and(|op| matches!(op, "timing_ab" | "settle"))
        {
            clean_args.remove("source_refs");
        }
        self.call_tool_inner(name, &clean_args).map(|result| {
            result.and_then(|mut output| {
                if !output.structured["report_id"].is_string() {
                    output.structured = saccade_core::report_links::decorate(&output.structured)?;
                } else {
                    output.structured =
                        saccade_core::report_links::migrate_linked(&output.structured);
                }
                Ok(output)
            })
        })
    }
    fn call_tool_inner(&self, name: &str, args: &Map<String, Value>) -> Option<ToolResult> {
        #[cfg(feature = "products")]
        if name == "saccade_products" {
            return Some(self.product_tool(args));
        }
        if !matches!(
            name,
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
        if name == "saccade_measure"
            && args.get("operation").and_then(Value::as_str) == Some("arms_check")
        {
            return Some(
                crate::arms_mcp::call(&self.policy, Value::Object(args.clone())).map(
                    |structured| ToolOutput {
                        structured,
                        text: "Arm identity validation; no pixel verdict.".into(),
                        images: vec![],
                    },
                ),
            );
        }
        // wave11
        if args
            .get("operation")
            .and_then(Value::as_str)
            .is_some_and(|op| crate::wave11_mcp::handles(name, op))
        {
            return Some(
                crate::wave11_mcp::call(&self.policy, args).map(|structured| ToolOutput {
                    structured,
                    text: "External timing, settling or report links.".into(),
                    images: vec![],
                }),
            );
        }
        // wave10
        if args
            .get("operation")
            .and_then(Value::as_str)
            .is_some_and(|op| crate::wave10_mcp::handles(name, op))
        {
            return Some(
                crate::wave10_mcp::call(&self.policy, args).map(|structured| ToolOutput {
                    structured,
                    text: "Embedded schema or empirical repeat noise evidence.".into(),
                    images: vec![],
                }),
            );
        }
        // wave9
        if args
            .get("operation")
            .and_then(Value::as_str)
            .is_some_and(|op| crate::wave9_mcp::handles(name, op))
        {
            return Some(
                crate::wave9_mcp::call(&self.policy, args).map(|structured| ToolOutput {
                    structured,
                    text: "Local reference evidence or an immutable blind trial receipt.".into(),
                    images: vec![],
                }),
            );
        }
        // wave8: rooted image records, no download/provider authority from arguments.
        if name == "saccade_measure"
            && args.get("operation").and_then(Value::as_str) == Some("analyze_media")
        {
            return Some(
                crate::media_cmd::mcp(&self.policy, &Value::Object(args.clone())).map(
                    |structured| ToolOutput {
                        structured,
                        text:
                            "Versioned media record; skipped and failed sections remain explicit."
                                .into(),
                        images: vec![],
                    },
                ),
            );
        }
        // wave7
        if args
            .get("operation")
            .and_then(Value::as_str)
            .is_some_and(|op| crate::wave7_mcp::handles(name, op))
        {
            return Some(crate::wave7_mcp::call(&self.policy, args).map(|structured| ToolOutput { structured, text: "Standalone vision observation; model evidence is advisory and replay is explicitly labelled.".into(), images: vec![] }));
        }
        Some(self.local_tool(name, args))
    }
    #[cfg(feature = "ai")]
    fn provider_review(&self, args: &Map<String, Value>) -> ToolResult {
        #[cfg(feature = "assist")]
        if args
            .get("operation")
            .and_then(Value::as_str)
            .is_some_and(|op| {
                [
                    "explain",
                    "audit-mask",
                    "check-ui",
                    "batch-submit",
                    "batch-status",
                    "batch-collect",
                ]
                .contains(&op)
            })
        {
            return self.assist_review(args);
        }
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
        let _scope = saccade_core::root_policy::io::scope(&self.policy);
        let file = self.existing_file("artifact", &require_str(args, "artifact")?)?;
        crate::review_cmd::guard_case_inputs(&file, &self.policy)
            .map_err(|e| CliError::new("unsafe_path", e.message))?;
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
    fn wave6_tool(&self, args: &Map<String, Value>) -> ToolResult {
        use clap::ValueEnum;
        let operation = require_str(args, "operation")?;
        #[cfg(feature = "print")]
        if operation == "print_compare" {
            reject_unknown(
                args,
                &[
                    "operation",
                    "reference",
                    "capture",
                    "out",
                    "input_profile",
                    "output_profile",
                    "tac_limit",
                    "dpi",
                    "small_text_points",
                ],
            )?;
            let a = self.existing_file("reference", &require_str(args, "reference")?)?;
            let b = self.existing_file("capture", &require_str(args, "capture")?)?;
            let input_profile = arg_str(args, "input_profile")?
                .map(|p| self.existing_file("input_profile", &p))
                .transpose()?;
            let output_profile = arg_str(args, "output_profile")?
                .map(|p| self.existing_file("output_profile", &p))
                .transpose()?;
            let mut inputs = vec![a.as_path(), b.as_path()];
            inputs.extend(input_profile.as_deref());
            inputs.extend(output_profile.as_deref());
            let out = self.checked_out_dir(&require_str(args, "out")?, &inputs)?;
            crate::general_cmd::prepare_out(&out, &inputs)?;
            let options = saccade_print::Options {
                input_profile,
                output_profile,
                tac_limit: arg_f64(args, "tac_limit")?
                    .ok_or_else(|| CliError::usage("tac_limit is required"))?,
                dpi: arg_f64(args, "dpi")?.ok_or_else(|| CliError::usage("dpi is required"))?,
                small_text_points: arg_f64(args, "small_text_points")?.unwrap_or(12.),
            };
            let value = saccade_print::compare(&a, &b, &out, &options)
                .map_err(|e| CliError::new(e.code(), e.to_string()))?;
            saccade_core::report_links::write(&out.join("saccade-print.v1.json"), &value)?;
            return Ok(ToolOutput {
                structured: value,
                text: "ICC-managed CMYK raster evidence; no press certification.".into(),
                images: vec![],
            });
        }
        if operation == "capabilities" {
            reject_unknown(args, &["operation"])?;
            return Ok(ToolOutput {
                structured: crate::capability_cmd::catalogue(),
                text: "Comparison families and conditional/deferred availability.".into(),
                images: Vec::new(),
            });
        }
        // Document rendering uses resource isolation; it is not a syscall/filesystem sandbox.
        if operation == "documents_compare" {
            reject_unknown(
                args,
                &[
                    "operation",
                    "reference",
                    "capture",
                    "out",
                    "dpi",
                    "threshold",
                ],
            )?;
            let a = self.existing_file("reference", &require_str(args, "reference")?)?;
            let b = self.existing_file("capture", &require_str(args, "capture")?)?;
            let out = self.checked_out_dir(&require_str(args, "out")?, &[&a, &b])?;
            let options = crate::general_cmd::CompareArgs {
                page_map: None,
                dpi: arg_f64(args, "dpi")?,
                ..Default::default()
            };
            let value = crate::documents_cmd::measure(
                &a,
                &b,
                &out,
                &options,
                arg_f64(args, "threshold")?.unwrap_or(0.02),
                crate::MetricArg::Mean,
            )?;
            let file = out.join(format!("{}.json", saccade_core::general::documents::SCHEMA));
            return Ok(ToolOutput {structured:json!({"schema":saccade_core::general::RESULT_SCHEMA,"mode":operation,"verdict":value["verdict"],"data":{"schema":value["schema"],"counts":value["counts"]},"artifacts":[{"path":saccade_core::paths::record(&file,&self.root,false)}],"next_actions":[]}),text:"Rendered pages at declared density; page errors and missingness remain failures.".into(),images:Vec::new()});
        }
        if operation == "embedding_export_inputs" {
            reject_unknown(args, &["operation", "dir", "model", "out"])?;
            let mut paths = std::collections::BTreeMap::new();
            let dir = self.resolve("dir", &require_str(args, "dir")?)?;
            self.input_tree(&dir)?;
            let operator = self.operator_model(args, true)?;
            let model = operator.model.clone().ok_or_else(|| {
                CliError::usage("model contract id required (or configure an embedding contract)")
            })?;
            let out = self.checked_out_dir(&require_str(args, "out")?, &[&dir, &model])?;
            paths.insert("dir".into(), dir);
            paths.insert("model".into(), model);
            let value = crate::embedding_cmd::measure(&operation, &paths, 10, &out)?;
            let file = crate::general_cmd::persist_document(&value, &out)?;
            return Ok(ToolOutput {
                structured: json!({"schema":saccade_core::general::RESULT_SCHEMA,"mode":operation,"verdict":value["verdict"],"data":{"schema":value["schema"],"counts":value["counts"]},"artifacts":[{"path":saccade_core::paths::record(&file,&self.root,false)}],"next_actions":[]}),
                text: "Exact preprocessing tensors; no model execution or qualification.".into(),
                images: Vec::new(),
            });
        }
        if operation == "compare_question" {
            reject_unknown(
                args,
                &[
                    "operation",
                    "reference",
                    "capture",
                    "out",
                    "question",
                    "align",
                    "resample",
                    "threshold",
                    "model",
                    "cache",
                    "library",
                    "reference_source",
                    "capture_source",
                ],
            )?;
            let mut paths = std::collections::BTreeMap::new();
            for key in ["reference", "capture", "reference_source", "capture_source"] {
                if let Some(p) = arg_str(args, key)? {
                    let path = self.resolve(key, &p)?;
                    if path.is_dir() {
                        self.input_tree(&path)?;
                    } else if !path.is_file() {
                        return Err(CliError::io("question input unavailable"));
                    }
                    paths.insert(key, path);
                }
            }
            let same_content = arg_str(args, "question")?.as_deref() == Some("same-content");
            let operator = self.operator_model(args, same_content)?;
            for (key, value) in [
                ("model", &operator.model),
                ("cache", &operator.cache),
                ("library", &operator.library),
            ] {
                if let Some(p) = value {
                    paths.insert(key, p.clone());
                }
            }
            let reference = paths
                .get("reference")
                .ok_or_else(|| CliError::usage("reference required"))?;
            let capture = paths
                .get("capture")
                .ok_or_else(|| CliError::usage("capture required"))?;
            let out = self.checked_out_dir(
                &require_str(args, "out")?,
                &paths.values().map(PathBuf::as_path).collect::<Vec<_>>(),
            )?;
            let question =
                crate::capability_cmd::Question::from_str(&require_str(args, "question")?, false)
                    .map_err(CliError::usage)?;
            let options = crate::general_cmd::CompareArgs {
                page_map: None,
                dpi: None,
                question: Some(question),
                align: arg_str(args, "align")?
                    .map(|v| {
                        crate::general_cmd::Align::from_str(&v, false).map_err(CliError::usage)
                    })
                    .transpose()?,
                resample: arg_str(args, "resample")?
                    .map(|v| {
                        crate::general_cmd::Resample::from_str(&v, false).map_err(CliError::usage)
                    })
                    .transpose()?,
                model: paths.get("model").cloned(),
                cache: paths.get("cache").cloned(),
                library: paths.get("library").cloned(),
                reference_source: paths.get("reference_source").cloned(),
                capture_source: paths.get("capture_source").cloned(),
                ocr_contract: None,
            };
            crate::capability_cmd::validate(&options)?;
            if question == crate::capability_cmd::Question::SameContent {
                crate::embedding_cmd::authorize_runtime(
                    options
                        .library
                        .as_deref()
                        .ok_or_else(|| CliError::usage("same-content requires library"))?,
                )?;
            }
            if question == crate::capability_cmd::Question::SameRender && options.align.is_none() {
                let mut mapped = Map::new();
                mapped.insert("baseline_dir".into(), json!(reference));
                mapped.insert("capture_dir".into(), json!(capture));
                mapped.insert("out_dir".into(), json!(out));
                if let Some(v) = args.get("threshold") {
                    mapped.insert("threshold".into(), v.clone());
                }
                let mut result = self.tool_compare(&mapped)?;
                let report = crate::read_report(&out.join(saccade_core::report::REPORT_FILE_NAME))?;
                let choice = crate::capability_cmd::record_render(&report, &out, &options)?;
                result.structured["data"]["pipeline_choice"] = choice;
                return Ok(result);
            }
            let value = if question == crate::capability_cmd::Question::SameRender {
                let mut value = crate::general_cmd::compare_document(
                    reference,
                    capture,
                    &out,
                    &options,
                    arg_f64(args, "threshold")?.unwrap_or(0.02),
                    crate::MetricArg::Mean,
                )?;
                value["pipeline"]["selected_question"] = json!("same-render");
                value["pipeline"]["command"] = json!("compare --align");
                value
            } else {
                crate::capability_cmd::measure(
                    reference,
                    capture,
                    &out,
                    &options,
                    arg_f64(args, "threshold")?,
                )?
            };
            let file = crate::general_cmd::persist_document(&value, &out)?;
            return Ok(ToolOutput {
                structured: json!({"schema":saccade_core::general::RESULT_SCHEMA,"mode":"compare_question","verdict":value["verdict"],"data":{"schema":value["schema"],"counts":value["counts"]},"artifacts":[{"path":saccade_core::paths::record(&file,&self.root,false)}],"next_actions":[]}),
                text: "Explicit comparison question recorded; no fallback to a different family."
                    .into(),
                images: Vec::new(),
            });
        }
        if operation == "inspect_image" {
            reject_unknown(
                args,
                &[
                    "operation",
                    "image",
                    "out",
                    "include_gps",
                    "hash_index",
                    "text_source",
                    "output_size",
                    "crop",
                ],
            )?;
            let image = self.existing_file("image", &require_str(args, "image")?)?;
            let hash_index = arg_str(args, "hash_index")?
                .map(|p| self.existing_file("hash_index", &p))
                .transpose()?;
            let text_source = arg_str(args, "text_source")?
                .map(|p| self.existing_file("text_source", &p))
                .transpose()?;
            let mut inputs = vec![image.as_path()];
            inputs.extend(hash_index.as_deref());
            inputs.extend(text_source.as_deref());
            let out = self.checked_out_dir(&require_str(args, "out")?, &inputs)?;
            let crop = args
                .get("crop")
                .map(|v| {
                    v.as_array()
                        .filter(|a| a.len() == 4)
                        .ok_or_else(|| CliError::usage("crop needs four integers"))?
                        .iter()
                        .map(|v| {
                            v.as_u64()
                                .and_then(|v| u32::try_from(v).ok())
                                .ok_or_else(|| CliError::usage("crop coordinates must be u32"))
                        })
                        .collect::<Result<Vec<_>, CliError>>()
                })
                .transpose()?;
            let value = crate::inspect_image_cmd::imported(
                image,
                out.clone(),
                arg_bool(args, "include_gps")?.unwrap_or(false),
                hash_index,
                text_source,
                arg_strings(args, "output_size")?,
                crop,
            )?;
            let file = crate::general_cmd::persist_document(&value, &out)?;
            return Ok(ToolOutput{structured:json!({"schema":saccade_core::general::RESULT_SCHEMA,"mode":operation,"verdict":"unknown","data":{"schema":value["schema"],"counts":value["counts"]},"artifacts":[{"path":saccade_core::paths::record(&file,&self.root,false)}],"next_actions":[]}),text:"Single-image indicators; offline credentials when compiled, GPS opt-in; no authenticity verdict.".into(),images:Vec::new()});
        }
        if operation == "assess" {
            reject_unknown(args, &["operation", "image", "compare_to", "out"])?;
            let image = self.existing_file("image", &require_str(args, "image")?)?;
            let before = arg_str(args, "compare_to")?
                .map(|p| self.existing_file("compare_to", &p))
                .transpose()?;
            let mut inputs = vec![image.as_path()];
            inputs.extend(before.as_deref());
            let out = self.checked_out_dir(&require_str(args, "out")?, &inputs)?;
            let value = crate::assess_cmd::measure(&image, before.as_deref(), &out)?;
            let file = crate::general_cmd::persist_document(&value, &out)?;
            return Ok(ToolOutput{structured:json!({"schema":saccade_core::general::RESULT_SCHEMA,"mode":operation,"verdict":value["verdict"],"data":{"schema":value["schema"],"counts":value["counts"]},"artifacts":[{"path":saccade_core::paths::record(&file,&self.root,false)}],"next_actions":[]}),text:"Content-dependent quality indicators and optional paired deltas; no heuristic quality verdict.".into(),images:Vec::new()});
        }
        // Imported text observations are data; MCP never executes a supplied program.
        // OCR lane: request-bound provider fixtures only; no live egress authority in tool input.
        if operation == "document_text" {
            reject_unknown(
                args,
                &[
                    "operation",
                    "a",
                    "b",
                    "response_a",
                    "response_b",
                    "model",
                    "pages",
                    "expect_text",
                    "out",
                ],
            )?;
            let a = self.existing_file("a", &require_str(args, "a")?)?;
            let b = self.existing_file("b", &require_str(args, "b")?)?;
            let ra = self.existing_file("response_a", &require_str(args, "response_a")?)?;
            let rb = self.existing_file("response_b", &require_str(args, "response_b")?)?;
            let out = self.checked_out_dir(&require_str(args, "out")?, &[&a, &b, &ra, &rb])?;
            let pages = if let Some(p) = args.get("pages") {
                serde_json::from_value::<Vec<u32>>(p.clone())?
            } else {
                vec![0]
            };
            let value = crate::document_ocr_cmd::imported(
                [a, b],
                [ra, rb],
                require_str(args, "model")?,
                pages,
                arg_strings(args, "expect_text")?,
                &out,
            )?;
            let file = crate::general_cmd::persist_document(&value, &out)?;
            return Ok(ToolOutput {
                structured: json!({"schema":saccade_core::general::RESULT_SCHEMA,"mode":operation,"verdict":value["verdict"],"data":{"schema":value["schema"],"counts":value["counts"]},"artifacts":[{"path":saccade_core::paths::record(&file,&self.root,false)}],"next_actions":[]}),
                text: "Page-structured OCR fixtures compared; no live calls or readability proof."
                    .into(),
                images: Vec::new(),
            });
        }
        if operation == "text" {
            reject_unknown(
                args,
                &[
                    "operation",
                    "a",
                    "b",
                    "a_source",
                    "b_source",
                    "out",
                    "expect_text",
                    "readable_confidence",
                    "moved_px",
                ],
            )?;
            let a = self.existing_file("a", &require_str(args, "a")?)?;
            let b = self.existing_file("b", &require_str(args, "b")?)?;
            let sa = self.existing_file("a_source", &require_str(args, "a_source")?)?;
            let sb = self.existing_file("b_source", &require_str(args, "b_source")?)?;
            let out = self.checked_out_dir(&require_str(args, "out")?, &[&a, &b, &sa, &sb])?;
            let value = crate::text_cmd::imported(
                [a, b],
                [sa, sb],
                out.clone(),
                arg_strings(args, "expect_text")?,
                [
                    arg_f64(args, "readable_confidence")?.unwrap_or(80.),
                    arg_f64(args, "moved_px")?.unwrap_or(3.),
                ],
            )?;
            let file = crate::general_cmd::persist_document(&value, &out)?;
            return Ok(ToolOutput{structured:json!({"schema":saccade_core::general::RESULT_SCHEMA,"mode":operation,"verdict":value["verdict"],"data":{"schema":value["schema"],"counts":value["counts"]},"artifacts":[{"path":saccade_core::paths::record(&file,&self.root,false)}],"next_actions":[]}),text:"OCR/source observations compared; missing text and confidence are uncertain evidence.".into(),images:Vec::new()});
        }
        // Model/runtime reads and cache writes use the same root authority.
        if matches!(
            operation.as_str(),
            "similar" | "index_build" | "index_query" | "embedding_calibrate"
        ) {
            let extra = match operation.as_str() {
                "similar" => vec!["a", "b"],
                "index_build" => vec!["dir"],
                "embedding_calibrate" => vec!["corpus"],
                _ => vec!["index", "image"],
            };
            let mut keys = vec!["operation", "out", "model", "cache", "library"];
            keys.extend(extra.iter().copied());
            if operation == "index_query" {
                keys.push("top");
            }
            reject_unknown(args, &keys)?;
            // Model, cache and runtime come from operator configuration; a request may only
            // name a registry contract id or restate a configured location, never a path.
            let operator = self.operator_model(args, true)?;
            let mut paths = std::collections::BTreeMap::new();
            for (key, value) in [
                ("model", &operator.model),
                ("library", &operator.library),
                ("cache", &operator.cache),
            ] {
                paths.insert(
                    key.to_owned(),
                    value.clone().ok_or_else(|| {
                        CliError::usage(format!("{key} is not configured by the operator"))
                    })?,
                );
            }
            for key in extra.iter().copied() {
                let path = self.resolve(key, &require_str(args, key)?)?;
                if path.is_dir() {
                    self.input_tree(&path)?;
                } else if !path.is_file() {
                    return Err(CliError::io("embedding input unavailable"));
                }
                paths.insert(key.to_owned(), path);
            }
            let out = self.checked_out_dir(
                &require_str(args, "out")?,
                &paths.values().map(PathBuf::as_path).collect::<Vec<_>>(),
            )?;
            let top = args
                .get("top")
                .map(|v| {
                    v.as_u64()
                        .filter(|n| (1..=100).contains(n))
                        .ok_or_else(|| CliError::usage("top must be 1..100"))
                })
                .transpose()?
                .unwrap_or(10) as usize;
            crate::embedding_cmd::authorize_runtime(
                paths
                    .get("library")
                    .ok_or_else(|| CliError::usage("library required"))?,
            )?;
            let value = crate::embedding_cmd::measure(&operation, &paths, top, &out)?;
            let file = crate::general_cmd::persist_document(&value, &out)?;
            return Ok(ToolOutput{structured:json!({"schema":saccade_core::general::RESULT_SCHEMA,"mode":operation,"verdict":value["verdict"],"data":{"schema":value["schema"],"counts":value["counts"]},"artifacts":[{"path":saccade_core::paths::record(&file,&self.root,false)}],"next_actions":[]}),text:"Conditional embedding evidence; no bundled export or calibration qualification.".into(),images:Vec::new()});
        }
        if matches!(operation.as_str(), "hash" | "dedupe") {
            reject_unknown(
                args,
                &["operation", "files", "out", "algorithm", "threshold"],
            )?;
            let files = arg_strings(args, "files")?
                .iter()
                .map(|p| {
                    let path = self.resolve("files", p)?;
                    if path.is_dir() {
                        self.input_tree(&path)?;
                    } else if !path.is_file() {
                        return Err(CliError::io("hash input must exist"));
                    }
                    Ok(path)
                })
                .collect::<Result<Vec<_>, CliError>>()?;
            let out = self.checked_out_dir(
                &require_str(args, "out")?,
                &files.iter().map(PathBuf::as_path).collect::<Vec<_>>(),
            )?;
            let algorithm = crate::hash_cmd::Algorithm::from_str(
                arg_str(args, "algorithm")?.as_deref().unwrap_or("phash"),
                false,
            )
            .map_err(CliError::usage)?;
            let threshold = args
                .get("threshold")
                .map(|v| {
                    v.as_u64()
                        .filter(|v| *v <= 64)
                        .ok_or_else(|| CliError::usage("threshold must be 0..64"))
                })
                .transpose()?
                .unwrap_or(6) as u32;
            if operation == "hash"
                && (args.contains_key("algorithm") || args.contains_key("threshold"))
            {
                return Err(CliError::usage(
                    "algorithm and threshold apply only to dedupe",
                ));
            }
            let value = crate::hash_cmd::measure(
                &files,
                &out,
                (operation == "dedupe").then_some((algorithm.into(), threshold)),
            )?;
            let file = crate::general_cmd::persist_document(&value, &out)?;
            return Ok(ToolOutput{structured:json!({"schema":saccade_core::general::RESULT_SCHEMA,"mode":operation,"verdict":value["verdict"],"data":{"schema":value["schema"],"counts":value["counts"]},"artifacts":[{"path":saccade_core::paths::record(&file,&self.root,false)}],"next_actions":[]}),text:"Perceptual hash candidate retrieval; originals unchanged, collisions require review.".into(),images:Vec::new()});
        }
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
                ..Default::default()
            },
            arg_f64(args, "threshold")?.unwrap_or(0.02),
            metric,
        )?;
        let file = crate::general_cmd::persist_document(&value, &out)?;
        Ok(ToolOutput{structured:json!({"schema":saccade_core::general::RESULT_SCHEMA,"mode":"registered_compare","verdict":value["verdict"],"data":{"schema":value["schema"],"counts":value["counts"]},"artifacts":[{"path":saccade_core::paths::record(&file,&self.root,false)}],"next_actions":[]}),text:"Registered comparison over explicit geometric overlap; inspect model, residual and exclusion mask.".into(),images:Vec::new()})
    }

    fn local_tool(&self, name: &str, args: &Map<String, Value>) -> ToolResult {
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
                        "gpu_clock_map",
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
            ("saccade_measure", op @ ("safety" | "a11y" | "a11y_auto")) => {
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
        if saccade_core::report_links::original_schema(
            result.structured["schema"].as_str().unwrap_or_default(),
        ) != "saccade-result.v2"
        {
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
    #[cfg(feature = "products")] product_network: bool,
    #[cfg(feature = "products")] product_notifications: bool,
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
    #[cfg(feature = "products")]
    {
        server.product_network = product_network;
        server.product_notifications = product_notifications;
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
            if operation == "ablate" {
                schema["properties"]["base_repeats"] =
                    json!({"type":"array","minItems":1,"maxItems":128,"items":{"type":"string"}});
                schema["properties"]["arm_repeats"] = json!({"type":"array","minItems":1,"maxItems":128,"items":{"type":"object","additionalProperties":false,"required":["label","repeats"],"properties":{"label":{"type":"string"},"repeats":{"type":"array","minItems":1,"maxItems":128,"items":{"type":"string"}}}}});
                schema["required"] = json!(["out_dir"]);
                schema["anyOf"] = json!([{"required":["base_dir","arm_dirs"]},{"required":["base_repeats","arm_repeats"]}]);
            }
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
    measures.push(json!({"type":"object","properties":{"operation":{"const":"noise","type":"string"},"dirs":{"type":"array","items":{"type":"string"},"minItems":2},"out":{"type":"string"},"kind":{"type":"string","enum":if cfg!(feature="graphics"){vec!["image","performance"]}else{vec!["image"]}},"margin":{"type":"number","minimum":1},"metric":{"enum":["mean","p95","p99","max"]},"gpu_clock_map":{"type":"string"},"perf_name":{"type":"string"},"perf_noise":{"type":"string"},"perf_noise_k":{"type":"number","exclusiveMinimum":0},"perf_resolution_ms":{"type":"number","exclusiveMinimum":0},"perf_resolution_ticks":{"type":"integer","minimum":1},"perf_min_delta_ms":{"type":"number","minimum":0},"perf_min_delta_pct":{"type":"number","minimum":0}},"required":["operation","dirs","out"],"additionalProperties":false}));
    #[cfg(feature="graphics")]
    measures.push(json!({"type":"object","properties":{"operation":{"const":"bisect","type":"string"},"runs":{"type":"array","items":{"type":"string"},"minItems":2},"good":{"type":"string"},"out":{"type":"string"},"threshold":{"type":"number","minimum":0,"maximum":1},"metric":{"enum":["mean","p95","p99","max"]},"entry":{"type":"string"}},"required":["operation","runs","out"],"additionalProperties":false}));
    #[cfg(feature = "prechecks")]
    for old in crate::precheck_mcp::schemas() {
        let mut schema = old["inputSchema"].clone();
        if let Some(props) = schema["properties"].as_object_mut() {
            props.insert("operation".into(),json!({"const":if old["name"]=="saccade_safety"{"safety"}else if old["name"]=="saccade_a11y_auto"{"a11y_auto"}else{"a11y"},"type":"string"}));
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
    // wave7
    measures.extend(crate::wave7_mcp::measure_schemas());
    // wave9
    measures.push(crate::arms_mcp::schema());
    measures.extend(crate::wave9_mcp::schemas());
    // wave10
    measures.extend(crate::wave10_mcp::schemas());
    // wave11
    measures.extend(crate::wave11_mcp::schemas());
    for measure in &mut measures {
        if let Some(props) = measure["properties"].as_object_mut() {
            props.insert(
                "source_refs".into(),
                json!({"type":"array","maxItems":128,"items":{"type":"string"}}),
            );
        }
    }
    // wave8
    measures.push(crate::media_cmd::mcp_schema());
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
        json!({"name":name,"description":description,"inputSchema":{"type":"object","oneOf":variants},"outputSchema":{"type":"object","properties":{"schema":{"const":saccade_core::report_links::linked_schema("saccade-result.v2")}},"required":["schema"]},"annotations":{"destructiveHint":false,"openWorldHint":false}})
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
        #[cfg(feature = "assist")]
        if let Some(variants) = tool["inputSchema"]["oneOf"].as_array_mut() {
            variants.extend(assist::schemas());
        }
        tool["annotations"]["openWorldHint"] = json!(true);
        list.push(tool);
    }
    #[cfg(feature = "products")]
    if let Some(list) = schemas.as_array_mut() {
        list.push(products::schema());
    }
    if let Some(list) = schemas.as_array_mut() {
        list.push(crate::general_cmd::tool_schema());
    }
    // wave7
    if let Some(list) = schemas.as_array_mut()
        && let Some(inspect) = list.iter_mut().find(|t| t["name"] == "saccade_inspect")
    {
        if let Some(variants) = inspect["inputSchema"]["oneOf"].as_array_mut() {
            variants.push(crate::wave7_mcp::inspect_schema());
        }
        inspect["outputSchema"] = json!({"type":"object","properties":{"schema":{"enum":[saccade_core::report_links::linked_schema("saccade-result.v2"),saccade_core::report_links::linked_schema("saccade-model-status.v1")]}},"required":["schema"]});
    }
    schemas
}

#[cfg(feature = "assist")]
#[path = "mcp_assist.rs"]
mod assist;
