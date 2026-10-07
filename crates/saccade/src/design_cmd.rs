//! Generic design-source evidence with a Figma REST adapter and recorded fixtures.
use crate::{agent::CliError, product_io as io};
use clap::{Args, Subcommand};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

pub(crate) const PULL_SCHEMA: &str = "saccade-design-pull.v1";
pub(crate) const MAP_SCHEMA: &str = "saccade-design-map.v1";
pub(crate) const CAPTURE_SCHEMA: &str = "saccade-design-captures.v1";
pub(crate) const COMPARE_SCHEMA: &str = "saccade-design-report.v1";
#[derive(Args)]
pub(crate) struct DesignArgs {
    #[command(subcommand)]
    operation: Operation,
}
#[derive(Subcommand)]
enum Operation {
    /// Export mapped design frames and tokens, cached by file version.
    Pull {
        mapping: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        cache: Option<PathBuf>,
        #[arg(long)]
        fixture_dir: Option<PathBuf>,
        /// Export scale factor, 0.01-4 (default 1; 2 = twice the pixel size).
        #[arg(long, default_value_t = 1.0)]
        scale: f64,
        #[arg(long)]
        json: bool,
    },
    /// Compare mapped frames and implementation captures; retain expected layout differences.
    Compare {
        mapping: PathBuf,
        #[arg(long)]
        pull: PathBuf,
        #[arg(long)]
        captures: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        config: Option<PathBuf>,
        #[arg(long,value_parser=["none","translation"],default_value="translation")]
        align: String,
        #[arg(long)]
        json: bool,
    },
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Mapping {
    pub(crate) schema: String,
    pub(crate) frames: Vec<FrameMap>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FrameMap {
    pub(crate) file_key: String,
    pub(crate) node_id: String,
    pub(crate) url: Option<String>,
    pub(crate) test_name: Option<String>,
    pub(crate) selector: Option<String>,
    pub(crate) viewport: [u32; 2],
    #[serde(default)]
    pub(crate) css_tokens: Vec<Value>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Token {
    pub(crate) name: String,
    pub(crate) color: Option<[f64; 3]>,
    pub(crate) font_family: Option<String>,
    pub(crate) font_size: Option<f64>,
    pub(crate) source: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Export {
    pub(crate) file_key: String,
    pub(crate) node_id: String,
    pub(crate) version: String,
    pub(crate) path: Option<PathBuf>,
    pub(crate) sha256: Option<String>,
    pub(crate) error: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Pull {
    pub(crate) schema: String,
    pub(crate) source: String,
    pub(crate) scale: f64,
    pub(crate) frames: Vec<Export>,
    pub(crate) tokens: Vec<Token>,
    pub(crate) degradations: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Capture {
    pub(crate) file_key: String,
    pub(crate) node_id: String,
    pub(crate) viewport: [u32; 2],
    pub(crate) url: Option<String>,
    pub(crate) test_name: Option<String>,
    pub(crate) selector: Option<String>,
    pub(crate) status: String,
    pub(crate) path: Option<PathBuf>,
    pub(crate) sha256: Option<String>,
    pub(crate) error: Option<String>,
    #[serde(default)]
    pub(crate) css_tokens: Vec<Value>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Captures {
    pub(crate) schema: String,
    pub(crate) mapping_sha256: String,
    pub(crate) entries: Vec<Capture>,
}
fn digest(bytes: impl AsRef<[u8]>) -> String {
    format!("{:x}", Sha256::digest(bytes.as_ref()))
}
pub(crate) fn frame_id(key: &str, node: &str) -> String {
    digest(format!("{key}:{node}"))
}
fn valid_id(text: &str) -> bool {
    !text.is_empty()
        && text.len() <= 128
        && text
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b':'))
}
pub(crate) fn mapping(path: &Path) -> Result<Mapping, CliError> {
    let map: Mapping = io::json(path)?;
    if map.schema != MAP_SCHEMA {
        return Err(CliError::new("version_skew", "unsupported design mapping"));
    }
    if map.frames.is_empty() || map.frames.len() > 64 {
        return Err(CliError::usage("design mapping needs 1..64 frames"));
    }
    let mut ids = BTreeSet::new();
    for frame in &map.frames {
        if !valid_id(&frame.file_key)
            || !valid_id(&frame.node_id)
            || !ids.insert(frame_id(&frame.file_key, &frame.node_id))
            || frame.viewport.iter().any(|n| *n < 8 || *n > 16384)
            || (frame.url.is_none()
                && frame
                    .test_name
                    .as_deref()
                    .is_none_or(|s| s.trim().is_empty()))
        {
            return Err(CliError::usage("invalid or duplicate design frame mapping"));
        }
        if let Some(url) = &frame.url {
            io::url(url)?;
        }
    }
    Ok(map)
}
/// Adapter boundary: provider JSON and image bytes remain untrusted data.
pub(crate) trait DesignSource {
    fn json(
        &self,
        file: &str,
        kind: &str,
        nodes: &[String],
        scale: f64,
    ) -> Result<(u16, Value), CliError>;
    fn image(&self, url: &str, id: &str) -> Result<Vec<u8>, CliError>;
}
struct Figma {
    token: String,
}
impl DesignSource for Figma {
    fn json(
        &self,
        file: &str,
        kind: &str,
        nodes: &[String],
        scale: f64,
    ) -> Result<(u16, Value), CliError> {
        let mut endpoint = io::url(&format!(
            "https://api.figma.com/v1/{}",
            match kind {
                "file" => format!("files/{file}"),
                "images" => format!("images/{file}"),
                "styles" => format!("files/{file}/styles"),
                "variables" => format!("files/{file}/variables/local"),
                _ => return Err(CliError::usage("unknown design source operation")),
            }
        ))?;
        if kind == "images" {
            endpoint
                .query_pairs_mut()
                .append_pair("ids", &nodes.join(","))
                .append_pair("scale", &scale.to_string())
                .append_pair("format", "png");
        }
        let response = io::request(
            "GET",
            endpoint.as_str(),
            &[("X-Figma-Token", &self.token)],
            None,
        )?;
        if response.status == 403 && kind == "variables" {
            return Ok((403, Value::Null));
        }
        if !(200..300).contains(&response.status) {
            return Err(CliError::new(
                "design_http",
                format!(
                    "design provider HTTP {} (credentials redacted)",
                    response.status
                ),
            ));
        }
        let value = serde_json::from_slice(&response.bytes)
            .map_err(|_| CliError::new("design_response", "invalid design provider JSON"))?;
        Ok((response.status, value))
    }
    fn image(&self, url: &str, _id: &str) -> Result<Vec<u8>, CliError> {
        if io::url(url)?.scheme() != "https" {
            return Err(CliError::new(
                "design_response",
                "design download requires HTTPS",
            ));
        }
        Ok(io::get(url, &[])?.bytes)
    }
}
pub(crate) struct Fixtures {
    pub(crate) root: PathBuf,
}
impl DesignSource for Fixtures {
    fn json(
        &self,
        file: &str,
        kind: &str,
        _nodes: &[String],
        _scale: f64,
    ) -> Result<(u16, Value), CliError> {
        let value: Value = io::json(&self.root.join(file).join(format!("{kind}.json")))?;
        if value["fixture_status"] == 403 {
            Ok((403, Value::Null))
        } else {
            Ok((200, value))
        }
    }
    fn image(&self, _url: &str, id: &str) -> Result<Vec<u8>, CliError> {
        io::read(&self.root.join("images").join(format!("{id}.png")))
    }
}
fn rgb(value: &Value) -> Option<[f64; 3]> {
    let color = [
        value["r"].as_f64()?,
        value["g"].as_f64()?,
        value["b"].as_f64()?,
    ];
    (color
        .iter()
        .all(|n| n.is_finite() && (0.0..=1.0).contains(n))
        && value["a"].as_f64().is_none_or(|a| a == 1.0))
    .then_some(color)
}
fn tree_tokens(
    node: &Value,
    style_names: &BTreeMap<String, String>,
    out: &mut Vec<Token>,
    budget: &mut usize,
    depth: usize,
) -> Result<(), CliError> {
    *budget += 1;
    if *budget > 100_000 || depth > 128 {
        return Err(CliError::new(
            "input_budget",
            "design tree exceeds node/depth bound",
        ));
    }
    for kind in ["fill", "text"] {
        let Some(name) = node["styles"][kind]
            .as_str()
            .and_then(|id| style_names.get(id))
        else {
            continue;
        };
        let color = if kind == "fill" {
            node["fills"]
                .as_array()
                .and_then(|fills| {
                    fills.iter().find(|v| {
                        v["type"] == "SOLID"
                            && v["visible"] != false
                            && v["opacity"].as_f64().is_none_or(|v| v == 1.0)
                    })
                })
                .and_then(|fill| rgb(&fill["color"]))
        } else {
            None
        };
        out.push(Token {
            name: name.clone(),
            color,
            font_family: if kind == "text" {
                node["style"]["fontFamily"].as_str().map(str::to_owned)
            } else {
                None
            },
            font_size: if kind == "text" {
                node["style"]["fontSize"]
                    .as_f64()
                    .filter(|n| n.is_finite() && *n > 0.0)
            } else {
                None
            },
            source: format!("published_{kind}_style_document_node"),
        });
    }
    if let Some(children) = node["children"].as_array() {
        for child in children {
            tree_tokens(child, style_names, out, budget, depth + 1)?;
        }
    }
    Ok(())
}
fn extract_tokens(file: &Value, styles: &Value, variables: &Value) -> Result<Vec<Token>, CliError> {
    let names = styles["meta"]["styles"]
        .as_array()
        .map(|s| {
            s.iter()
                .filter_map(|style| {
                    Some((
                        style["key"].as_str()?.to_owned(),
                        style["name"].as_str()?.to_owned(),
                    ))
                })
                .collect::<BTreeMap<_, _>>()
        })
        .unwrap_or_default();
    let mut names = names;
    if let Some(styles) = file["styles"].as_object() {
        for (id, style) in styles {
            if let Some(name) = style["name"].as_str() {
                names.insert(id.clone(), name.into());
            }
        }
    }
    let mut out = Vec::new();
    tree_tokens(&file["document"], &names, &mut out, &mut 0, 0)?;
    if let Some(variables) = variables["meta"]["variables"].as_object() {
        for value in variables.values() {
            if value["resolvedType"] != "COLOR" {
                continue;
            }
            if let (Some(name), Some(modes)) =
                (value["name"].as_str(), value["valuesByMode"].as_object())
            {
                for (mode, color) in modes {
                    out.push(Token {
                        name: name.into(),
                        color: rgb(color),
                        font_family: None,
                        font_size: None,
                        source: format!("variable_mode:{mode}"),
                    });
                }
            }
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name).then(a.source.cmp(&b.source)));
    out.dedup_by(|a, b| {
        a.name == b.name
            && a.color == b.color
            && a.font_family == b.font_family
            && a.font_size == b.font_size
    });
    Ok(out)
}
pub(crate) fn pull(
    map: &Mapping,
    out: &Path,
    cache: &Path,
    scale: f64,
    source: &dyn DesignSource,
    source_name: &str,
) -> Result<Pull, CliError> {
    if !scale.is_finite() || !(0.01..=4.0).contains(&scale) {
        return Err(CliError::usage("design scale must be 0.01..4"));
    }
    if out.exists() {
        return Err(CliError::new(
            "not_empty_out_dir",
            "design pull output must be new",
        ));
    }
    let mut files: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for frame in &map.frames {
        files
            .entry(frame.file_key.clone())
            .or_default()
            .push(frame.node_id.clone());
    }
    let mut result = Pull {
        schema: PULL_SCHEMA.into(),
        source: source_name.into(),
        scale,
        frames: Vec::new(),
        tokens: Vec::new(),
        degradations: Vec::new(),
    };
    std::fs::create_dir_all(out.join("frames"))
        .map_err(|_| CliError::io("cannot create design output"))?;
    for (key, nodes) in files {
        let (_, file) = source.json(&key, "file", &nodes, scale)?;
        let version = file["version"]
            .as_str()
            .filter(|v| !v.is_empty())
            .ok_or_else(|| CliError::new("design_response", "design file version missing"))?
            .to_owned();
        let cache_path = cache.join(digest(format!(
            "{source_name}:{key}:{version}:{scale}:{}",
            nodes.join(",")
        )));
        let cached = cache_path.join("pull.json");
        if cached.is_file() {
            let pull: Pull = io::json(&cached)?;
            if pull.schema != PULL_SCHEMA
                || pull.scale != scale
                || pull
                    .frames
                    .iter()
                    .any(|e| e.file_key != key || e.version != version)
            {
                return Err(CliError::new("design_cache", "cache identity mismatch"));
            }
            for frame in &pull.frames {
                if let (Some(path), Some(hash)) = (&frame.path, &frame.sha256) {
                    let data = bounded_asset(&cache_path, path, hash)?;
                    std::fs::write(out.join(path), data)
                        .map_err(|_| CliError::io("cannot copy cached frame"))?;
                }
            }
            result.frames.extend(pull.frames);
            result.tokens.extend(pull.tokens);
            result.degradations.extend(pull.degradations);
            continue;
        }
        let (_, styles) = source.json(&key, "styles", &nodes, scale)?;
        let (status, variables) = source.json(&key, "variables", &nodes, scale)?;
        let degradation = if status == 403 {
            vec![format!(
                "{key}: variables forbidden (403); published styles retained"
            )]
        } else {
            Vec::new()
        };
        let (_, images) = source.json(&key, "images", &nodes, scale)?;
        let tokens = extract_tokens(&file, &styles, &variables)?;
        let mut local = Pull {
            schema: PULL_SCHEMA.into(),
            source: source_name.into(),
            scale,
            frames: Vec::new(),
            tokens,
            degradations: degradation,
        };
        std::fs::create_dir_all(cache_path.join("frames"))
            .map_err(|_| CliError::io("cannot create design cache"))?;
        for node in nodes {
            let id = frame_id(&key, &node);
            let mut export = Export {
                file_key: key.clone(),
                node_id: node.clone(),
                version: version.clone(),
                path: None,
                sha256: None,
                error: None,
            };
            let data = images["images"][&node]
                .as_str()
                .ok_or_else(|| CliError::new("design_response", "frame export URL missing"))
                .and_then(|url| source.image(url, &id))
                .and_then(|data| {
                    io::image(&data)?;
                    if image::guess_format(&data).ok() != Some(image::ImageFormat::Png) {
                        return Err(CliError::new("design_response", "frame export is not PNG"));
                    }
                    Ok(data)
                });
            match data {
                Ok(data) => {
                    let path = PathBuf::from(format!("frames/{id}.png"));
                    std::fs::write(out.join(&path), &data)
                        .map_err(|_| CliError::io("cannot write exported frame"))?;
                    std::fs::write(cache_path.join(&path), &data)
                        .map_err(|_| CliError::io("cannot cache exported frame"))?;
                    export.sha256 = Some(digest(&data));
                    export.path = Some(path);
                }
                Err(error) => export.error = Some(format!("{}: {}", error.code, error.message)),
            };
            local.frames.push(export);
        }
        // Failed exports must be retried, never promoted to a complete cache hit.
        if local.frames.iter().all(|e| e.error.is_none()) {
            io::write(&cached, &local)?;
        }
        result.frames.extend(local.frames);
        result.tokens.extend(local.tokens);
        result.degradations.extend(local.degradations);
    }
    io::write(&out.join("pull.json"), &result)?;
    Ok(result)
}
fn bounded_asset(root: &Path, path: &Path, hash: &str) -> Result<Vec<u8>, CliError> {
    if path.is_absolute()
        || path
            .components()
            .any(|c| !matches!(c, std::path::Component::Normal(_)))
    {
        return Err(CliError::new(
            "unsafe_path",
            "design asset path must be relative",
        ));
    }
    let root = root
        .canonicalize()
        .map_err(|_| CliError::io("design asset root missing"))?;
    let full = root
        .join(path)
        .canonicalize()
        .map_err(|_| CliError::io("design asset missing"))?;
    if !full.starts_with(root) {
        return Err(CliError::new("unsafe_path", "design asset escapes root"));
    }
    let data = io::read(&full)?;
    if digest(&data) != hash {
        return Err(CliError::new(
            "design_identity",
            "design asset content differs",
        ));
    }
    Ok(data)
}
fn css_color(text: &str) -> Option<[f64; 3]> {
    if let Some(hex) = text.strip_prefix('#')
        && hex.len() == 6
    {
        let value = u32::from_str_radix(hex, 16).ok()?;
        return Some([
            ((value >> 16) & 255) as f64 / 255.0,
            ((value >> 8) & 255) as f64 / 255.0,
            (value & 255) as f64 / 255.0,
        ]);
    }
    let body = text
        .strip_prefix("rgb(")
        .or_else(|| text.strip_prefix("rgba("))?
        .strip_suffix(')')?;
    let parts = body
        .split(|c: char| c == ',' || c == '/' || c.is_whitespace())
        .filter(|s| !s.is_empty())
        .map(str::parse::<f64>)
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    if parts.len() < 3
        || parts.len() > 4
        || parts
            .iter()
            .take(3)
            .any(|n| !n.is_finite() || !(0.0..=255.0).contains(n))
        || parts.get(3).is_some_and(|a| *a != 1.0)
    {
        return None;
    }
    Some([parts[0] / 255.0, parts[1] / 255.0, parts[2] / 255.0])
}
pub(crate) fn token_check(tokens: &[Token], css: &[Value]) -> Value {
    let mut findings = Vec::new();
    for token in tokens {
        let matches = css
            .iter()
            .filter(|v| v["name"].as_str() == Some(token.name.as_str()))
            .collect::<Vec<_>>();
        if matches.is_empty() {
            findings
                .push(json!({"name":token.name,"status":"unmatched_design","source":token.source}));
            continue;
        }
        for value in matches {
            let measured = value["color"].as_str().and_then(css_color);
            let delta = token.color.zip(measured).map(|(a, b)| {
                saccade_core::color::delta_e(
                    saccade_core::color::lab(a.map(saccade_core::color::linear)),
                    saccade_core::color::lab(b.map(saccade_core::color::linear)),
                )
            });
            let families = value["font_family"]
                .as_str()
                .unwrap_or("")
                .split(',')
                .map(|s| s.trim().trim_matches(['\'', '"']).to_lowercase())
                .collect::<Vec<_>>();
            let family = token
                .font_family
                .as_ref()
                .map(|font| families.first().is_some_and(|v| *v == font.to_lowercase()));
            let size = token
                .font_size
                .zip(
                    value["font_size"]
                        .as_str()
                        .and_then(|s| s.strip_suffix("px"))
                        .and_then(|s| s.parse::<f64>().ok())
                        .filter(|v| v.is_finite()),
                )
                .map(|(a, b)| b - a);
            findings.push(json!({"name":token.name,"source":token.source,"delta_e2000":delta,"colour_bucket":delta.map(|d|if d<1.0{"under_1"}else if d<3.0{"1_to_3"}else if d<10.0{"3_to_10"}else{"10_or_more"}),"font_family_matches":family,"font_size_delta_px":size,"unmatched_value":(token.color.is_none()&&token.font_family.is_none()&&token.font_size.is_none())||(token.color.is_some()&&delta.is_none())||(token.font_family.is_some()&&family!=Some(true))||(token.font_size.is_some()&&size.is_none())}));
        }
    }
    for value in css {
        if !tokens
            .iter()
            .any(|t| value["name"].as_str() == Some(t.name.as_str()))
        {
            findings.push(json!({"name":value["name"],"status":"unmatched_implementation"}));
        }
    }
    json!({"findings":findings,"meaning":"ΔE2000 buckets are measured sRGB differences, not universal acceptance thresholds; font family is declared CSS, not proof of the resolved rendered face"})
}
pub(crate) fn compare(
    map_path: &Path,
    pull_path: &Path,
    captures_path: &Path,
    out: &Path,
    config: &saccade_core::config::RunConfig,
    align: &str,
) -> Result<Value, CliError> {
    let map = mapping(map_path)?;
    let pull: Pull = io::json(pull_path)?;
    let captures: Captures = io::json(captures_path)?;
    if pull.schema != PULL_SCHEMA
        || captures.schema != CAPTURE_SCHEMA
        || captures.mapping_sha256 != digest(io::read(map_path)?)
    {
        return Err(CliError::new(
            "design_identity",
            "design/capture contract identity mismatch",
        ));
    }
    let temp = tempfile::tempdir().map_err(|_| CliError::io("design workspace unavailable"))?;
    let base = temp.path().join("design");
    let candidate = temp.path().join("implementation");
    std::fs::create_dir(&base).map_err(|_| CliError::io("design staging unavailable"))?;
    std::fs::create_dir(&candidate).map_err(|_| CliError::io("capture staging unavailable"))?;
    let mut failures = Vec::new();
    let mut css = Vec::new();
    let mut used = BTreeSet::new();
    for frame in &map.frames {
        let id = frame_id(&frame.file_key, &frame.node_id);
        let exports = pull
            .frames
            .iter()
            .filter(|e| e.file_key == frame.file_key && e.node_id == frame.node_id)
            .collect::<Vec<_>>();
        let entries = captures
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| {
                e.file_key == frame.file_key
                    && e.node_id == frame.node_id
                    && e.viewport == frame.viewport
                    && e.selector == frame.selector
                    && e.url == frame.url
                    && e.test_name == frame.test_name
            })
            .collect::<Vec<_>>();
        if entries.len() == 1 {
            used.insert(entries[0].0);
        }
        if exports.len() != 1 || entries.len() != 1 {
            failures.push(json!({"id":id,"error":"missing or ambiguous mapped frame/capture"}));
            continue;
        }
        used.insert(entries[0].0);
        let export = exports[0];
        let capture = entries[0].1;
        css.extend(capture.css_tokens.clone());
        for (side, root, path, hash, error, destination) in [
            (
                "design",
                pull_path.parent().unwrap_or(Path::new(".")),
                export.path.as_deref(),
                export.sha256.as_deref(),
                export.error.as_deref(),
                &base,
            ),
            (
                "implementation",
                captures_path.parent().unwrap_or(Path::new(".")),
                capture.path.as_deref(),
                capture.sha256.as_deref(),
                if capture.status == "captured" {
                    capture.error.as_deref()
                } else {
                    Some("capture failed")
                },
                &candidate,
            ),
        ] {
            let bytes = if error.is_some() {
                Err(CliError::new(
                    "capture_failure",
                    "design export or implementation capture failed",
                ))
            } else {
                path.zip(hash)
                    .ok_or_else(|| {
                        CliError::new("capture_failure", "frame/capture artifact missing")
                    })
                    .and_then(|(p, h)| bounded_asset(root, p, h))
            };
            match bytes {
                Ok(data) => std::fs::write(destination.join(format!("{id}.png")), data)
                    .map_err(|_| CliError::io("cannot stage design comparison"))?,
                Err(e) => {
                    failures.push(json!({"id":id,"side":side,"code":e.code,"error":e.message}))
                }
            }
        }
    }
    if used.len() != captures.entries.len() {
        return Err(CliError::new(
            "design_identity",
            "capture entries not covered uniquely by mapping",
        ));
    }
    let mut config = config.clone();
    config.fail_on_new = true;
    config.allow_empty = false;
    config.diagnostics.enabled = true;
    config.diagnostics.shift_detection = align == "translation";
    let report = saccade_core::run::run(&base, &candidate, &out.join("comparison"), &config)?;
    let alignment=report.entries.iter().map(|e|json!({"id":e.name,"status":e.status,"raw_metrics":e.metrics,"translation":e.diagnostics.as_ref().and_then(|d|d.shift.as_ref())})).collect::<Vec<_>>();
    let value = json!({"schema":COMPARE_SCHEMA,"verdict":if failures.is_empty()&&!report.is_regression(){"pass"}else{"regression"},"frames":alignment,"capture_failures":failures,"token_check":token_check(&pull.tokens,&css),"degradations":pull.degradations,"report":"comparison/index.html","layout_note":"Layout differences between a static frame and responsive implementation are expected. Raw metrics decide; translation compensation is diagnostic and never hides layout changes. Different dimensions remain explicit errors."});
    io::write(&out.join("saccade-design-report.v1.json"), &value)?;
    Ok(value)
}
pub(crate) fn pull_live(
    map: &Mapping,
    out: &Path,
    cache: &Path,
    scale: f64,
) -> Result<Pull, CliError> {
    let values = io::provider_env("figma")?;
    let token = values
        .get("FIGMA_TOKEN")
        .filter(|v| !v.is_empty())
        .ok_or_else(|| CliError::new("credentials", "FIGMA_TOKEN missing in user figma.env"))?
        .clone();
    pull(map, out, cache, scale, &Figma { token }, "figma")
}
pub(crate) fn run(args: DesignArgs) -> Result<u8, CliError> {
    match args.operation {
        Operation::Pull {
            mapping: map,
            out,
            cache,
            fixture_dir,
            scale,
            json,
        } => {
            let map = mapping(&map)?;
            let cache =
                cache.unwrap_or_else(|| saccade_core::local::default_cache_dir().join("design"));
            let source_is_fixture = fixture_dir.is_some();
            let source: Box<dyn DesignSource> = if let Some(root) = fixture_dir {
                Box::new(Fixtures { root })
            } else {
                let values = io::provider_env("figma")?;
                let token = values
                    .get("FIGMA_TOKEN")
                    .filter(|v| !v.is_empty())
                    .ok_or_else(|| {
                        CliError::new("credentials", "FIGMA_TOKEN missing in user figma.env")
                    })?
                    .clone();
                Box::new(Figma { token })
            };
            let pull = pull(
                &map,
                &out,
                &cache,
                scale,
                source.as_ref(),
                if source_is_fixture {
                    "recorded_fixture"
                } else {
                    "figma"
                },
            )?;
            io::emit(&pull, json, "design frames and tokens pulled")?;
            Ok(u8::from(pull.frames.iter().any(|e| e.error.is_some())))
        }
        Operation::Compare {
            mapping,
            pull,
            captures,
            out,
            config,
            align,
            json,
        } => {
            let value = compare(
                &mapping,
                &pull,
                &captures,
                &out,
                &crate::load_config(config.as_deref())?,
                &align,
            )?;
            io::emit(&value, json, "design comparison and token evidence written")?;
            Ok(u8::from(value["verdict"] != "pass"))
        }
    }
}
#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    #[test]
    fn tokens_report_colour_buckets_fonts_and_unmatched() {
        let tokens = vec![
            Token {
                name: "accent".into(),
                color: Some([1.0, 0.0, 0.0]),
                font_family: Some("Example Sans".into()),
                font_size: Some(16.0),
                source: "fixture".into(),
            },
            Token {
                name: "missing".into(),
                color: None,
                font_family: None,
                font_size: None,
                source: "fixture".into(),
            },
        ];
        let result = token_check(
            &tokens,
            &[
                json!({"name":"accent","color":"rgb(255, 0, 0)","font_family":"\"Example Sans\", sans-serif","font_size":"18px"}),
                json!({"name":"extra"}),
            ],
        );
        assert_eq!(result["findings"][0]["colour_bucket"], "under_1");
        assert_eq!(result["findings"][0]["font_family_matches"], true);
        assert_eq!(result["findings"][0]["font_size_delta_px"], 2.0);
        assert_eq!(result["findings"][1]["status"], "unmatched_design");
        assert_eq!(result["findings"][2]["status"], "unmatched_implementation");
    }
    #[test]
    fn variable_aliases_and_transparency_remain_unmatched() {
        let variables = json!({"meta":{"variables":{"a":{"name":"opaque","resolvedType":"COLOR","valuesByMode":{"default":{"r":0.2,"g":0.3,"b":0.4,"a":1}}},"b":{"name":"alias","resolvedType":"COLOR","valuesByMode":{"default":{"type":"VARIABLE_ALIAS","id":"a"}}}}}});
        let tokens =
            extract_tokens(&json!({"document":{}}), &json!({}), &variables).expect("tokens");
        assert_eq!(tokens.len(), 2);
        let styles=extract_tokens(&json!({"document":{"styles":{"fill":"colour","text":"type"},"fills":[{"type":"SOLID","color":{"r":1,"g":0,"b":0}}],"style":{"fontFamily":"Example Sans","fontSize":16}},"styles":{"colour":{"name":"accent"},"type":{"name":"body"}}}),&json!({}),&json!({})).expect("both style roles");
        assert_eq!(styles.len(), 2);
        assert_eq!(styles[1].font_family.as_deref(), Some("Example Sans"));
        assert_eq!(css_color("rgba(20,30,40,0.5)"), None);
    }
    #[test]
    #[ignore = "heavy: design-fixtures"]
    fn recorded_pull_degrades_and_caches_version() {
        let root = tempfile::tempdir().expect("root");
        let fixtures = root.path().join("fixtures");
        std::fs::create_dir_all(fixtures.join("fixture-file")).expect("dir");
        std::fs::create_dir(fixtures.join("images")).expect("images");
        let file = json!({"version":"v1","document":{"children":[{"styles":{"fill":"style1"},"fills":[{"type":"SOLID","color":{"r":1,"g":0,"b":0}}]}]},"styles":{"style1":{"name":"accent"}}});
        for (kind, value) in [
            ("file", file),
            ("styles", json!({"meta":{"styles":[]}})),
            ("variables", json!({"fixture_status":403})),
            (
                "images",
                json!({"images":{"1:2":"https://example.com/frame.png"}}),
            ),
        ] {
            io::write(
                &fixtures.join("fixture-file").join(format!("{kind}.json")),
                &value,
            )
            .expect("fixture");
        }
        let id = frame_id("fixture-file", "1:2");
        image::RgbImage::from_pixel(32, 32, image::Rgb([80, 90, 100]))
            .save(fixtures.join("images").join(format!("{id}.png")))
            .expect("png");
        let map = Mapping {
            schema: MAP_SCHEMA.into(),
            frames: vec![FrameMap {
                file_key: "fixture-file".into(),
                node_id: "1:2".into(),
                url: Some("https://example.com/".into()),
                test_name: None,
                selector: None,
                viewport: [32, 32],
                css_tokens: vec![],
            }],
        };
        let cache = root.path().join("cache");
        let source = Fixtures {
            root: fixtures.clone(),
        };
        let a = pull(
            &map,
            &root.path().join("a"),
            &cache,
            1.0,
            &source,
            "fixture",
        )
        .expect("pull");
        assert_eq!(a.degradations.len(), 1);
        assert_eq!(a.tokens.len(), 1);
        std::fs::remove_file(fixtures.join("fixture-file/styles.json")).expect("remove");
        let b = pull(
            &map,
            &root.path().join("b"),
            &cache,
            1.0,
            &source,
            "fixture",
        )
        .expect("cache hit");
        assert_eq!(a.frames[0].sha256, b.frames[0].sha256);
        let map_path = root.path().join("mapping.json");
        io::write(&map_path, &map).expect("mapping");
        let captures = Captures {
            schema: CAPTURE_SCHEMA.into(),
            mapping_sha256: digest(io::read(&map_path).expect("read")),
            entries: vec![Capture {
                file_key: "fixture-file".into(),
                node_id: "1:2".into(),
                viewport: [32, 32],
                url: Some("https://example.com/".into()),
                test_name: None,
                selector: None,
                status: "captured".into(),
                path: Some(a.frames[0].path.clone().expect("frame")),
                sha256: a.frames[0].sha256.clone(),
                error: None,
                css_tokens: vec![json!({"name":"accent","color":"rgb(255, 0, 0)"})],
            }],
        };
        let captures_path = root.path().join("a/captures.json");
        io::write(&captures_path, &captures).expect("captures");
        let report = compare(
            &map_path,
            &root.path().join("a/pull.json"),
            &captures_path,
            &root.path().join("report"),
            &saccade_core::config::RunConfig::default(),
            "translation",
        )
        .expect("compare");
        assert_eq!(report["verdict"], "pass");
        assert_eq!(
            report["token_check"]["findings"][0]["colour_bucket"],
            "under_1"
        );
    }
}
