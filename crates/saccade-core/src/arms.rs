//! Versioned arm identity and strict, pre-measurement comparison validity.
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// Producer fingerprint contract identifier.
pub const FINGERPRINT_SCHEMA: &str = "saccade-arm-fingerprint.v1";
/// Standalone validity result contract identifier.
pub const RESULT_SCHEMA: &str = "saccade-arms-check.v1";

/// A producer's optional identity declarations; strict mode requires all groups.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Fingerprint {
    /// Versioned contract identifier.
    pub schema: String,
    /// Executable and build declarations.
    pub producer: Option<Producer>,
    /// Prepared content declaration.
    pub inputs: Option<Inputs>,
    /// Execution and readiness declarations.
    pub run: Option<Run>,
}
/// Executable identity.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Producer {
    /// Content hash of the executable.
    pub binary: Option<String>,
    /// Profile, features and other build settings.
    pub build: Option<BTreeMap<String, Value>>,
}
/// Prepared inputs identity.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Inputs {
    /// Content or preparation-report hash binding preparer identity.
    pub identity: Option<String>,
}
/// Execution identity.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Run {
    /// Producer's mode name.
    pub mode: Option<String>,
    /// Explicitly selected environment and CLI settings (empty means none).
    pub env: Option<BTreeMap<String, Value>>,
    /// Independently evaluated readiness predicates.
    pub readiness: Option<Vec<Readiness>>,
    /// Shared baseline/candidate capture session.
    pub session: Option<String>,
}
/// One named readiness predicate and its observation.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Readiness {
    /// Predicate name and exact parameters.
    pub criterion: Criterion,
    /// Whether this predicate was satisfied.
    pub reached: bool,
    /// Measured value, not an inferred success.
    pub observed: Value,
}
/// Predicate identity.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Criterion {
    /// Stable name, unique within each arm.
    pub name: String,
    /// Parameters used to test readiness (empty means no parameters).
    pub parameters: BTreeMap<String, Value>,
}
/// A recorded field finding, with unknown values represented by null.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Finding {
    /// Canonical metadata path.
    pub key: String,
    /// Baseline declaration.
    pub baseline: Option<Value>,
    /// Candidate declaration.
    pub capture: Option<Value>,
    /// Stable reason: missing, difference, readiness_not_reached, malformed.
    pub reason: String,
}
/// Identity validity, independent of any pixel verdict.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComparisonResult {
    /// Complete identities differ only in declared variables or explicit ignores.
    ValidComparison,
    /// Missing or mismatched identity prevents a measurement verdict.
    InvalidComparison,
}
/// Validity only; this result never contains a pixel pass/fail verdict.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Check {
    /// Result contract version.
    pub schema: String,
    /// valid_comparison or invalid_comparison.
    pub result: ComparisonResult,
    /// 0 valid, 3 invalid difference/readiness, 4 missing identity.
    pub exit_code: u8,
    /// Offending fields, sorted by key.
    pub offending: Vec<Finding>,
    /// Explicit ignored tokens, including tokens matching no fields.
    pub ignore: Vec<String>,
    /// Ignored differences (missing identity is never ignored).
    pub ignored: Vec<Finding>,
    /// Declared variable tokens.
    pub vary: Vec<String>,
}
/// A source path in a capture or a sibling JSON file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    /// Optional sibling relative filename, otherwise the selected capture record.
    pub file: Option<String>,
    /// Dotted object path (exact flat keys are also supported).
    pub path: String,
}
/// Map a producer readiness flag to a named generic criterion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReadinessMap {
    /// Predicate name.
    pub name: String,
    /// Exact predicate parameters.
    #[serde(default)]
    pub parameters: BTreeMap<String, Value>,
    /// Source boolean flag.
    pub reached: Source,
    /// Source observation.
    pub observed: Source,
}
/// Generic field mappings, read from bounded TOML or JSON.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FingerprintMap {
    /// Canonical destination to producer source.
    #[serde(default)]
    pub fields: BTreeMap<String, Source>,
    /// Optional independent readiness predicates from producer flags.
    #[serde(default)]
    pub readiness: Vec<ReadinessMap>,
}
impl FingerprintMap {
    /// Load and validate a mapping file without interpreting project names.
    pub fn read(path: &Path) -> Result<Self> {
        let bytes = crate::evidence_quality::read(path, 1 << 20)?;
        let map: Self = if path.extension().is_some_and(|e| e == "json") {
            serde_json::from_slice(&bytes)?
        } else {
            toml::from_str(std::str::from_utf8(&bytes).map_err(|e| Error::Config(e.to_string()))?)
                .map_err(|e| Error::Config(e.to_string()))?
        };
        let siblings: BTreeSet<_> = map
            .fields
            .values()
            .chain(map.readiness.iter().flat_map(|r| [&r.reached, &r.observed]))
            .filter_map(|s| s.file.as_ref())
            .collect();
        if map.fields.len() > 128 || map.readiness.len() > 32 || siblings.len() > 32 {
            return Err(Error::Config(
                "fingerprint map exceeds field, predicate or sibling limit".into(),
            ));
        }
        for key in map.fields.keys() {
            if !matches!(
                key.as_str(),
                "producer.binary"
                    | "producer.build"
                    | "inputs.identity"
                    | "run.mode"
                    | "run.env"
                    | "run.session"
                    | "run.readiness[]"
            ) && !key.starts_with("producer.build.")
                && !key.starts_with("run.env.")
            {
                return Err(Error::Config(format!(
                    "unsupported fingerprint destination {key:?}"
                )));
            }
        }
        for source in map
            .fields
            .values()
            .chain(map.readiness.iter().flat_map(|r| [&r.reached, &r.observed]))
        {
            if source.path.is_empty() {
                return Err(Error::Config("mapping path must be nonempty".into()));
            }
            if let Some(file) = &source.file {
                safe_sibling(Path::new("."), file)?;
            }
        }
        let mut names = BTreeSet::new();
        for r in &map.readiness {
            if r.name.trim().is_empty() || !names.insert(&r.name) {
                return Err(Error::Config(
                    "readiness mapping names must be nonempty and unique".into(),
                ));
            }
        }
        Ok(map)
    }
}
fn safe_sibling(root: &Path, file: &str) -> Result<PathBuf> {
    if file.is_empty()
        || Path::new(file)
            .components()
            .any(|p| !matches!(p, std::path::Component::Normal(_)))
    {
        return Err(Error::Config(
            "mapping files must be sibling relative paths without traversal".into(),
        ));
    }
    let mut path = root.to_path_buf();
    for part in Path::new(file).components() {
        path.push(part);
        if std::fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err(Error::Config("mapping files cannot follow symlinks".into()));
        }
    }
    Ok(path)
}
fn raw(path: &Path) -> Result<Option<Value>> {
    if !path.exists() {
        return Ok(None);
    }
    let v = serde_json::from_slice(&crate::evidence_quality::read(path, 1 << 20)?)?;
    if !matches!(v, Value::Object(_)) {
        return Err(Error::Config("capture record must be a JSON object".into()));
    }
    Ok(Some(v))
}
fn lookup(value: &Value, path: &str) -> Option<Value> {
    if let Some(v) = value.get(path) {
        return Some(v.clone());
    }
    let mut v = value;
    for part in path.split('.') {
        v = v.get(part)?;
    }
    Some(v.clone())
}
/// Flatten metadata, preserving readiness as an array for named comparison.
pub fn flatten(prefix: &str, value: &Value, out: &mut crate::meta::Meta) {
    if ((prefix.is_empty()
        && value.get("schema").and_then(Value::as_str) == Some(FINGERPRINT_SCHEMA))
        || prefix == "fingerprint")
        && let Some(obj) = value.as_object()
    {
        for (key, v) in obj {
            flatten(
                if key == "schema" {
                    "fingerprint.schema"
                } else {
                    key
                },
                v,
                out,
            );
        }
        return;
    }
    if prefix == "run.readiness" {
        out.insert(prefix.into(), value.clone());
        return;
    }
    if let Value::Object(obj) = value {
        if obj.is_empty() {
            out.insert(prefix.into(), value.clone());
        }
        for (k, v) in obj {
            let key = if prefix.is_empty() {
                k.clone()
            } else {
                format!("{prefix}.{k}")
            };
            flatten(&key, v, out);
        }
    } else {
        out.insert(prefix.into(), value.clone());
    }
}
fn merge(a: &mut Value, b: Value) {
    if let (Some(aa), Some(bb)) = (a.as_object_mut(), b.as_object()) {
        for (k, v) in bb {
            if let Some(old) = aa.get_mut(k) {
                merge(old, v.clone());
            } else {
                aa.insert(k.clone(), v.clone());
            }
        }
    } else {
        *a = b;
    }
}
/// Load native metadata with per-image inheritance, then merge mapped siblings.
pub fn load(input: &Path, name: &str, map: Option<&FingerprintMap>) -> Result<crate::meta::Meta> {
    if input.extension().is_some_and(|e| e == "json") {
        let root = input.parent().unwrap_or(Path::new("."));
        return mapped(root, raw(input)?.unwrap_or(serde_json::json!({})), map);
    }
    let root = if input.is_dir() {
        input
    } else {
        input.parent().unwrap_or(Path::new("."))
    };
    let rel = if input.is_dir() {
        "__arm__"
    } else {
        input.file_name().and_then(|n| n.to_str()).unwrap_or("")
    };
    load_named(root, rel, name, map)
}
/// Read effective inherited metadata for an image below its capture root.
pub fn load_named(
    root: &Path,
    rel: &str,
    name: &str,
    map: Option<&FingerprintMap>,
) -> Result<crate::meta::Meta> {
    crate::meta::MetaOptions {
        name: name.into(),
        ..Default::default()
    }
    .checker()?;
    if Path::new(rel)
        .components()
        .any(|p| !matches!(p, std::path::Component::Normal(_)))
    {
        return Err(Error::Config(
            "image name must be relative without traversal".into(),
        ));
    }
    let mut primary = serde_json::json!({});
    let mut dir = root.to_path_buf();
    if let Some(v) = raw(&safe_sibling(&dir, name)?)? {
        merge(&mut primary, v);
    }
    let path = Path::new(rel);
    if let Some(parent) = path.parent() {
        for part in parent.components() {
            dir.push(part);
            if std::fs::symlink_metadata(&dir).is_ok_and(|m| m.file_type().is_symlink()) {
                return Err(Error::Config("metadata cannot follow symlinks".into()));
            }
            if let Some(v) = raw(&safe_sibling(&dir, name)?)? {
                merge(&mut primary, v);
            }
        }
    }
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    if let Some(v) = raw(&safe_sibling(&dir, &format!("{stem}.{name}"))?)? {
        merge(&mut primary, v);
    }
    mapped(root, primary, map)
}
fn mapped(root: &Path, primary: Value, map: Option<&FingerprintMap>) -> Result<crate::meta::Meta> {
    let mut meta = BTreeMap::new();
    flatten("", &primary, &mut meta);
    if let Some(map) = map {
        let mut files = BTreeMap::new();
        for source in map
            .fields
            .values()
            .chain(map.readiness.iter().flat_map(|r| [&r.reached, &r.observed]))
        {
            if let Some(file) = &source.file
                && !files.contains_key(file)
            {
                files.insert(file.clone(), raw(&safe_sibling(root, file)?)?);
            }
        }
        for (file, value) in &files {
            if let Some(value) = value {
                flatten(&format!("file:{file}"), value, &mut meta);
            }
        }
        // Source fields become canonical fields; unmapped metadata remains comparable.
        for source in map
            .fields
            .values()
            .chain(map.readiness.iter().flat_map(|r| [&r.reached, &r.observed]))
        {
            let key = if let Some(file) = &source.file {
                format!("file:{file}.{}", source.path)
            } else {
                source.path.clone()
            };
            meta.retain(|k, _| k != &key && !k.starts_with(&format!("{key}.")));
        }
        let get = |s: &Source| -> Option<Value> {
            let v = if let Some(file) = &s.file {
                files.get(file).and_then(Option::as_ref)
            } else {
                Some(&primary)
            };
            v.and_then(|v| lookup(v, &s.path))
        };
        meta.insert(
            "fingerprint.schema".into(),
            Value::String(FINGERPRINT_SCHEMA.into()),
        );
        for (key, source) in &map.fields {
            let key = key.trim_end_matches("[]");
            // A missing mapped source must not fall back to a stale native value.
            meta.retain(|k, _| k != key && !k.starts_with(&format!("{key}.")));
            if let Some(v) = get(source) {
                flatten(key, &v, &mut meta);
            }
        }
        if !map.readiness.is_empty() {
            meta.insert("run.readiness".into(), Value::Array(map.readiness.iter().map(|r| serde_json::json!({"criterion":{"name":r.name,"parameters":r.parameters},"reached":get(&r.reached),"observed":get(&r.observed)})).collect()));
        }
    }
    Ok(meta)
}
/// Exact token, dotted prefix/suffix token, or explicit glob matching.
pub fn matches(token: &str, key: &str) -> bool {
    token == key
        || key.starts_with(&format!("{token}."))
        || key.ends_with(&format!(".{token}"))
        || crate::config::compile_glob(token).is_ok_and(|g| g.is_match(key))
}
fn known(value: Option<&Value>) -> bool {
    value.is_some_and(|v| !v.is_null() && !v.as_str().is_some_and(|s| s.trim().is_empty()))
}
fn finding(key: &str, a: &crate::meta::Meta, b: &crate::meta::Meta, reason: &str) -> Finding {
    Finding {
        key: key.into(),
        baseline: a.get(key).cloned(),
        capture: b.get(key).cloned(),
        reason: reason.into(),
    }
}
pub(crate) fn readiness(meta: &mut crate::meta::Meta) {
    let records = match meta.remove("run.readiness") {
        Some(Value::Array(records)) => records,
        None | Some(Value::Null) => return,
        Some(value) => {
            meta.insert("run.readiness.malformed".into(), value);
            return;
        }
    };
    if records.is_empty() {
        return;
    }
    let mut names = BTreeSet::new();
    for r in records {
        let name = r
            .pointer("/criterion/name")
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty());
        let Some(name) = name else {
            meta.insert("run.readiness.malformed".into(), r);
            continue;
        };
        if !names.insert(name.to_owned()) {
            meta.insert("run.readiness.malformed".into(), r);
            continue;
        }
        let key = format!("run.readiness.{name}");
        for (field, v) in [
            ("criterion", r.get("criterion")),
            ("reached", r.get("reached")),
            ("observed", r.get("observed")),
        ] {
            meta.insert(format!("{key}.{field}"), v.cloned().unwrap_or(Value::Null));
        }
    }
}
/// Validate identities and every undeclared metadata difference, without pixels.
pub fn check(
    a: &crate::meta::Meta,
    b: &crate::meta::Meta,
    vary: &[String],
    ignore: &[String],
) -> Check {
    let (mut a, mut b) = (a.clone(), b.clone());
    let mut bad = Vec::new();
    for key in [
        "fingerprint.schema",
        "producer.binary",
        "inputs.identity",
        "run.mode",
        "run.session",
    ] {
        if !known(a.get(key)) || !known(b.get(key)) {
            bad.push(finding(key, &a, &b, "missing"));
        } else if !a[key].is_string()
            || !b[key].is_string()
            || (key == "fingerprint.schema"
                && (a[key] != FINGERPRINT_SCHEMA || b[key] != FINGERPRINT_SCHEMA))
        {
            bad.push(finding(key, &a, &b, "malformed"));
        }
    }
    for group in ["producer.build", "run.env"] {
        let present = |m: &crate::meta::Meta| {
            m.get(group).is_some_and(|v| {
                v.is_object()
                    && (group == "run.env" || v.as_object().is_some_and(|o| !o.is_empty()))
            }) || m
                .iter()
                .any(|(k, v)| k.starts_with(&format!("{group}.")) && known(Some(v)))
        };
        if !present(&a) || !present(&b) {
            bad.push(finding(group, &a, &b, "missing"));
        }
    }
    readiness(&mut a);
    readiness(&mut b);
    if !a.keys().any(|k| k.starts_with("run.readiness."))
        || !b.keys().any(|k| k.starts_with("run.readiness."))
    {
        bad.push(finding("run.readiness", &a, &b, "missing"));
    }
    let criterion_keys: BTreeSet<_> = a
        .keys()
        .chain(b.keys())
        .filter(|k| k.starts_with("run.readiness.") && k.ends_with(".criterion"))
        .collect();
    for key in criterion_keys {
        for m in [&a, &b] {
            if m.contains_key(key) {
                let prefix = key.trim_end_matches(".criterion");
                for suffix in ["reached", "observed"] {
                    let field = format!("{prefix}.{suffix}");
                    if !known(m.get(&field)) {
                        bad.push(finding(&field, &a, &b, "missing"));
                    }
                }
            }
        }
    }
    for m in [&a, &b] {
        if m.keys().any(|k| k.starts_with("run.readiness."))
            && !m
                .keys()
                .any(|k| k.starts_with("run.readiness.") && k.ends_with(".criterion"))
        {
            bad.push(finding("run.readiness", &a, &b, "malformed"));
        }
    }
    let keys: BTreeSet<_> = a.keys().chain(b.keys()).collect();
    let mut ignored = Vec::new();
    for key in keys {
        let av = a.get(key);
        let bv = b.get(key);
        let criterion_key = key
            .rsplit_once('.')
            .map(|(prefix, _)| format!("{prefix}.criterion"));
        let criteria_differ = key.starts_with("run.readiness.")
            && criterion_key
                .as_ref()
                .is_some_and(|k| a.contains_key(k) != b.contains_key(k));
        let reason = if key == "run.readiness.malformed" {
            Some("malformed")
        } else if criteria_differ {
            Some("readiness_criteria_mismatch")
        } else if !known(av) || !known(bv) {
            Some("missing")
        } else if key.starts_with("run.readiness.")
            && key.ends_with(".criterion")
            && [av, bv].iter().any(|v| {
                v.is_none_or(|v| {
                    !v.get("parameters").is_some_and(Value::is_object)
                        || v.get("name").and_then(Value::as_str)
                            != Some(
                                key.trim_start_matches("run.readiness.")
                                    .trim_end_matches(".criterion"),
                            )
                })
            })
        {
            Some("malformed")
        } else if key.starts_with("run.readiness.")
            && key.ends_with(".reached")
            && (av != Some(&Value::Bool(true)) || bv != Some(&Value::Bool(true)))
        {
            Some("readiness_not_reached")
        } else if av != bv {
            Some("difference")
        } else {
            None
        };
        if let Some(reason) = reason {
            let f = finding(key, &a, &b, reason);
            if reason == "difference"
                && key != "fingerprint.schema"
                && key != "run.session"
                && !(key.starts_with("run.readiness.") && key.ends_with(".criterion"))
                && ignore.iter().any(|t| matches(t, key))
            {
                ignored.push(f);
            } else if reason != "difference"
                || key == "fingerprint.schema"
                || (key.starts_with("run.readiness.") && key.ends_with(".criterion"))
                || !vary.iter().any(|t| matches(t, key))
            {
                bad.push(f);
            }
        }
    }
    bad.sort_by(|a, b| a.key.cmp(&b.key));
    bad.dedup_by(|a, b| a.key == b.key && a.reason == b.reason);
    let code = if bad.iter().any(|f| f.reason == "missing") {
        4
    } else if !bad.is_empty() {
        3
    } else {
        0
    };
    Check {
        schema: RESULT_SCHEMA.into(),
        result: if code == 0 {
            ComparisonResult::ValidComparison
        } else {
            ComparisonResult::InvalidComparison
        },
        exit_code: code,
        offending: bad,
        ignore: ignore.to_vec(),
        ignored,
        vary: vary.to_vec(),
    }
}
/// Load and check two captures using effective metadata settings.
pub fn check_paths(a: &Path, b: &Path, opts: &crate::meta::MetaOptions) -> Result<Check> {
    let map = opts
        .fingerprint_map
        .as_deref()
        .map(FingerprintMap::read)
        .transpose()?;
    Ok(check(
        &load(a, &opts.name, map.as_ref())?,
        &load(b, &opts.name, map.as_ref())?,
        &opts.intended,
        &opts.ignore,
    ))
}
/// Refuse strict comparisons before output or measurements, including per-image overrides.
pub fn enforce(a: &Path, b: &Path, cfg: &crate::config::RunConfig) -> Result<()> {
    if !cfg.meta.require_valid_arms {
        return Ok(());
    }
    let checked = validate_paths(a, b, cfg)?;
    if checked.exit_code != 0 {
        return Err(Error::InvalidComparison(Box::new(checked)));
    }
    Ok(())
}
/// Validate every selected image pair, retaining all offending field names.
pub fn validate_paths(a: &Path, b: &Path, cfg: &crate::config::RunConfig) -> Result<Check> {
    let map = cfg
        .meta
        .fingerprint_map
        .as_deref()
        .map(FingerprintMap::read)
        .transpose()?;
    let mut checks = Vec::new();
    if a.is_dir() && b.is_dir() {
        let aa = crate::run::collect_images(a)?;
        let bb = crate::run::collect_images(b)?;
        let keys: BTreeSet<_> = aa.files.keys().chain(bb.files.keys()).collect();
        let ignores = cfg
            .ignore
            .iter()
            .map(|s| crate::config::compile_glob(s))
            .collect::<Result<Vec<_>>>()?;
        let entries = cfg
            .entries
            .iter()
            .map(|s| crate::config::compile_glob(s))
            .collect::<Result<Vec<_>>>()?;
        for key in keys {
            if ignores.iter().any(|g| g.is_match(key))
                || (!entries.is_empty() && !entries.iter().any(|g| g.is_match(key)))
            {
                continue;
            }
            let mut c = check(
                &load_named(a, key, &cfg.meta.name, map.as_ref())?,
                &load_named(b, key, &cfg.meta.name, map.as_ref())?,
                &cfg.meta.intended,
                &cfg.meta.ignore,
            );
            for f in &mut c.offending {
                f.key = format!("{key}:{}", f.key);
            }
            checks.push(c);
        }
        if checks.is_empty() {
            checks.push(check_paths(a, b, &cfg.meta)?);
        }
    } else {
        checks.push(check_paths(a, b, &cfg.meta)?);
    }
    let mut merged = checks.remove(0);
    for c in checks {
        merged.offending.extend(c.offending);
        merged.ignored.extend(c.ignored);
        merged.exit_code = merged.exit_code.max(c.exit_code);
    }
    if merged.exit_code != 0 {
        merged.result = ComparisonResult::InvalidComparison;
    }
    merged.offending.sort_by(|a, b| a.key.cmp(&b.key));
    merged.offending.dedup();
    merged.ignored.sort_by(|a, b| a.key.cmp(&b.key));
    merged.ignored.dedup();
    Ok(merged)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use serde_json::json;
    fn arm() -> crate::meta::Meta {
        let mut m = BTreeMap::new();
        flatten(
            "",
            &json!({"fingerprint":{"schema":FINGERPRINT_SCHEMA,"producer":{"binary":"sha256:abc","build":{"profile":"debug","features":[]}},"inputs":{"identity":"sha256:def"},"run":{"mode":"fixed","env":{},"session":"one","readiness":[{"criterion":{"name":"settled","parameters":{"epsilon":0.01}},"reached":true,"observed":0.001}]}}}),
            &mut m,
        );
        m
    }
    #[test]
    fn declared_variable_is_valid() {
        let a = arm();
        let mut b = a.clone();
        b.insert("run.mode".into(), json!("alternate"));
        assert_eq!(check(&a, &b, &["mode".into()], &[]).exit_code, 0);
    }
    #[test]
    fn undeclared_binary_refuses() {
        let a = arm();
        let mut b = a.clone();
        b.insert("producer.binary".into(), json!("sha256:changed"));
        let c = check(&a, &b, &[], &[]);
        assert_eq!(c.exit_code, 3);
        assert_eq!(c.offending[0].key, "producer.binary");
    }
    #[test]
    fn missing_content_never_equals_even_when_varied_or_ignored() {
        let mut a = arm();
        a.remove("inputs.identity");
        let c = check(&a, &a, &["inputs".into()], &["inputs".into()]);
        assert_eq!(c.exit_code, 4);
        assert_eq!(c.offending[0].key, "inputs.identity");
        assert_eq!(
            check(&BTreeMap::new(), &BTreeMap::new(), &[], &[]).exit_code,
            4
        );
    }
    #[test]
    fn unreached_readiness_refuses_even_if_varied_or_ignored() {
        let a = arm();
        let mut b = a.clone();
        b.get_mut("run.readiness").unwrap()[0]["reached"] = json!(false);
        let c = check(&a, &b, &["run.*".into()], &["run.*".into()]);
        assert_eq!(c.exit_code, 3);
        assert!(
            c.offending
                .iter()
                .any(|f| f.reason == "readiness_not_reached")
        );
    }
    #[test]
    fn different_named_criteria_refuse() {
        let a = arm();
        let mut b = a.clone();
        b.get_mut("run.readiness").unwrap()[0]["criterion"]["name"] = json!("receiver_ready");
        assert_eq!(
            check(&a, &b, &["run.*".into()], &["run.*".into()]).exit_code,
            3
        );
    }
    #[test]
    fn different_criterion_parameters_refuse() {
        let a = arm();
        let mut b = a.clone();
        b.get_mut("run.readiness").unwrap()[0]["criterion"]["parameters"]["epsilon"] = json!(0.5);
        assert_eq!(
            check(&a, &b, &["run.*".into()], &["run.*".into()]).exit_code,
            3
        );
    }
    #[test]
    fn session_requires_declaration() {
        let a = arm();
        let mut b = a.clone();
        b.insert("run.session".into(), json!("two"));
        assert_eq!(check(&a, &b, &[], &[]).exit_code, 3);
        assert_eq!(check(&a, &b, &[], &["session".into()]).exit_code, 3);
        assert_eq!(check(&a, &b, &["session".into()], &[]).exit_code, 0);
    }
    #[test]
    fn readiness_order_is_irrelevant_and_duplicates_are_invalid() {
        let mut a = arm();
        let r = a["run.readiness"][0].clone();
        let mut s = r.clone();
        s["criterion"]["name"] = json!("ready");
        a.insert("run.readiness".into(), json!([r, s]));
        let mut b = a.clone();
        b.get_mut("run.readiness")
            .unwrap()
            .as_array_mut()
            .unwrap()
            .reverse();
        assert_eq!(check(&a, &b, &[], &[]).exit_code, 0);
        b.get_mut("run.readiness").unwrap()[0] = r;
        assert_eq!(check(&a, &b, &[], &[]).exit_code, 3);
    }
    #[test]
    fn malformed_or_incomplete_readiness_never_proves_validity() {
        let a = arm();
        let mut b = a.clone();
        b.insert("run.readiness".into(), json!({"ready":true}));
        assert_eq!(check(&b, &b, &[], &[]).exit_code, 3);
        b = a.clone();
        b.get_mut("run.readiness").unwrap()[0]
            .as_object_mut()
            .unwrap()
            .remove("observed");
        assert_eq!(check(&b, &b, &[], &[]).exit_code, 4);
    }
    #[test]
    fn token_matching_and_explicit_ignores() {
        for (t, k) in [
            ("mode", "run.mode"),
            ("run", "run.mode"),
            ("run.*", "run.mode"),
            ("*.mode", "run.mode"),
            ("run.mode", "run.mode"),
        ] {
            assert!(matches(t, k));
        }
        assert!(!matches("mode", "run.model"));
        let a = arm();
        let mut b = a.clone();
        b.insert("generated_at".into(), json!("today"));
        assert_eq!(check(&a, &b, &[], &[]).exit_code, 4); // unknown on one side cannot be ignored
        let mut a = a;
        a.insert("generated_at".into(), json!("yesterday"));
        assert_eq!(check(&a, &b, &[], &[]).exit_code, 3);
        let c = check(&a, &b, &[], &["generated_at".into(), "unused".into()]);
        assert_eq!(c.exit_code, 0);
        assert_eq!(c.ignore.len(), 2);
        assert_eq!(c.ignored.len(), 1);
    }
    #[test]
    fn nested_override_is_checked_before_outputs() {
        let tmp = tempfile::tempdir().unwrap();
        let a = tmp.path().join("a");
        let b = tmp.path().join("b");
        for root in [&a, &b] {
            std::fs::create_dir_all(root.join("nested")).unwrap();
            std::fs::write(
                root.join("nested/frame.png"),
                b"not decoded during preflight",
            )
            .unwrap();
            std::fs::write(
                root.join("saccade-meta.json"),
                serde_json::to_vec(&arm()).unwrap(),
            )
            .unwrap();
        }
        std::fs::write(
            b.join("nested/frame.saccade-meta.json"),
            br#"{"producer.binary":"sha256:different"}"#,
        )
        .unwrap();
        let mut cfg = crate::config::RunConfig::default();
        cfg.meta.require_valid_arms = true;
        let out = tmp.path().join("out");
        let Err(Error::InvalidComparison(c)) = crate::run::run(&a, &b, &out, &cfg) else {
            panic!("expected refusal")
        };
        assert_eq!(c.exit_code, 3);
        assert!(!out.exists());
    }
    #[test]
    fn mapped_siblings_and_missing_source_are_visible() {
        let tmp = tempfile::tempdir().unwrap();
        let primary = tmp.path().join("capture.json");
        std::fs::write(&primary,br#"{"exe":{"hash":"sha256:a","profile":"debug"},"content":"sha256:c","mode":"fixed","session":"one"}"#).unwrap();
        std::fs::write(
            tmp.path().join("ready.json"),
            br#"{"ready":true,"value":0.001,"env":{}}"#,
        )
        .unwrap();
        let map:FingerprintMap=serde_json::from_value(json!({"fields":{"producer.binary":{"path":"exe.hash"},"producer.build.profile":{"path":"exe.profile"},"inputs.identity":{"path":"content"},"run.mode":{"path":"mode"},"run.session":{"path":"session"},"run.env":{"file":"ready.json","path":"env"}},"readiness":[{"name":"settled","parameters":{},"reached":{"file":"ready.json","path":"ready"},"observed":{"file":"ready.json","path":"value"}}]})).unwrap();
        let a = load(&primary, "saccade-meta.json", Some(&map)).unwrap();
        assert_eq!(check(&a, &a, &[], &[]).exit_code, 0);
        assert!(!a.contains_key("exe.hash"));
        std::fs::remove_file(tmp.path().join("ready.json")).unwrap();
        let b = load(&primary, "saccade-meta.json", Some(&map)).unwrap();
        assert_eq!(check(&a, &b, &[], &[]).exit_code, 4);
    }
}
