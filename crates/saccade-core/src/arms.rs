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
/// A recorded field finding; explicit states distinguish null from absence.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Finding {
    /// Canonical metadata path.
    pub key: String,
    /// Baseline declaration.
    pub baseline: Option<Value>,
    /// Candidate declaration.
    pub capture: Option<Value>,
    /// Baseline state: missing, null or value.
    #[serde(default)]
    pub baseline_state: String,
    /// Candidate state: missing, null or value.
    #[serde(default)]
    pub capture_state: String,
    /// Stable reason: missing, difference, readiness_not_reached, malformed.
    pub reason: String,
    /// Tokens covering this finding, with the exact destination or source name matched.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub token_matches: Vec<TokenMatch>,
}
/// One explicit token match; both names are reported when both match.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenMatch {
    /// Literal vary or ignore token.
    pub token: String,
    /// Whether the matched name is a canonical destination or producer source.
    pub via: MatchedName,
    /// Exact name tested against the token, without a directory image prefix.
    pub name: String,
}
/// Origin of a name matched by an explicit token.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MatchedName {
    /// Canonical metadata destination.
    Destination,
    /// Producer source path from the fingerprint map.
    Source,
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
    /// Explicitly waived findings, including missing fields and equal nulls.
    pub ignored: Vec<Finding>,
    /// Differences covered by vary tokens, with matched names and both values.
    #[serde(default)]
    pub covered_by_vary: Vec<Finding>,
    /// Differences covered by explicit field derivations, with both values.
    #[serde(default)]
    pub covered_by_derivation: Vec<Finding>,
    /// Explicitly permitted matched unreached predicates with both observations.
    #[serde(default)]
    pub allowed_unreached: Vec<AllowedUnreached>,
    /// Record discovery diagnostics, including exact searched filenames.
    #[serde(default)]
    pub diagnostics: Vec<String>,
    /// Effective map comparison mode, including any CLI override.
    #[serde(default)]
    pub compare: CompareMode,
    /// Unmapped fields; excluded in mapped_only and compared in all mode.
    #[serde(default)]
    pub unmapped: KeySummary,
    /// Fields explicitly excluded by the map's outcome globs in either mode.
    #[serde(default)]
    pub outcomes: KeySummary,
    /// Declared variable tokens.
    pub vary: Vec<String>,
}
/// Field selection for arm validity checks.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompareMode {
    /// Compare every effective metadata field (the compatibility default).
    #[default]
    All,
    /// Compare only mapped destinations, readiness and declared derived fields.
    MappedOnly,
}
/// Bounded, sorted union of excluded effective keys from both arms.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeySummary {
    /// Total distinct keys, including keys omitted from the list.
    pub count: usize,
    /// At most 64 keys, sorted lexicographically.
    pub keys: Vec<String>,
    /// Whether the key list omits any keys.
    pub truncated: bool,
}
impl KeySummary {
    fn finish(&mut self) {
        self.keys.sort();
        self.keys.dedup();
        self.count = self.keys.len();
        self.truncated = self.count > 64;
        self.keys.truncate(64);
    }
}
impl Check {
    fn finish(&mut self) {
        self.unmapped.finish();
        self.outcomes.finish();
    }
}
/// An explicit readiness exception; it never claims convergence.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AllowedUnreached {
    /// Exact criterion name.
    pub criterion: String,
    /// Baseline observation, retained even when null.
    pub baseline_observed: Value,
    /// Candidate observation.
    pub capture_observed: Value,
}
/// Policy for intentionally unconverged captures.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnreachedPolicy {
    /// Ordinary strict refusal.
    #[default]
    Refuse,
    /// Both false flags and exact matching observations/criteria are required.
    Matched,
}
/// A source path in a capture or a sibling JSON file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    /// Optional sibling relative filename, otherwise the selected capture record.
    pub file: Option<String>,
    /// Dotted object/array path with zero-based numeric indices; exact flat keys take precedence.
    pub path: String,
    /// Effective metadata keys computed from this mapped field.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub derives: Vec<String>,
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
    /// Explicit opt-in for identical intentionally unreached observations.
    #[serde(default)]
    pub unreached_policy: UnreachedPolicy,
    /// Source boolean flag.
    pub reached: Source,
    /// Source observation.
    pub observed: Source,
}
/// Generic field mappings, read from bounded TOML or JSON.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FingerprintMap {
    /// Field selection; all preserves the historical comparison behaviour.
    #[serde(default)]
    pub compare: CompareMode,
    /// Globs matching effective metadata keys that are never compared.
    #[serde(default)]
    pub outcomes: Vec<String>,
    /// Ordered arm-root JSON records merged before inherited/per-image sidecars.
    #[serde(default)]
    pub record_files: Vec<String>,
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
        if map.fields.len() > 128
            || map.readiness.len() > 32
            || siblings.len() > 32
            || map.record_files.len() > 32
        {
            return Err(Error::Config(
                "fingerprint map exceeds field, predicate or sibling limit".into(),
            ));
        }
        if map.outcomes.len() > 128 {
            return Err(Error::Config("outcomes requires at most 128 globs".into()));
        }
        for pattern in &map.outcomes {
            if pattern.trim().is_empty() {
                return Err(Error::Config("outcome globs must be nonempty".into()));
            }
            crate::config::compile_glob(pattern)?;
        }
        for file in &map.record_files {
            safe_sibling(Path::new("."), file)?;
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
            ) && !key.starts_with("inputs.")
                && !key.starts_with("producer.build.")
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
            if source.path.contains(['*', '?', '[', '{']) {
                return Err(Error::Config(format!(
                    "mapping source path {:?}: wildcards are unsupported; use a dotted path with an explicit numeric array index",
                    source.path
                )));
            }
            if let Some(file) = &source.file {
                safe_sibling(Path::new("."), file)?;
            }
        }
        for source in map.fields.values() {
            if source.derives.len() > 128
                || source
                    .derives
                    .iter()
                    .any(|k| k.trim().is_empty() || k.contains(['*', '?', '[', '{']))
            {
                return Err(Error::Config(
                    "derives requires at most 128 exact nonempty keys per field".into(),
                ));
            }
        }
        let mut names = BTreeSet::new();
        for r in &map.readiness {
            if !r.reached.derives.is_empty() || !r.observed.derives.is_empty() {
                return Err(Error::Config(
                    "derives is supported only in field mappings".into(),
                ));
            }
            if r.name.trim().is_empty() || !names.insert(&r.name) {
                return Err(Error::Config(
                    "readiness mapping names must be nonempty and unique".into(),
                ));
            }
        }
        Ok(map)
    }
}
impl FingerprintMap {
    fn names(&self, key: &str) -> bool {
        key == "fingerprint.schema"
            || self.fields.iter().any(|(dest, source)| {
                let dest = dest.trim_end_matches("[]");
                key == dest
                    || key.starts_with(&format!("{dest}."))
                    || source
                        .derives
                        .iter()
                        .any(|d| key == d || key.starts_with(&format!("{d}.")))
            })
            || (!self.readiness.is_empty() && key.starts_with("run.readiness."))
    }
    fn outcome_globs(&self) -> Vec<globset::GlobMatcher> {
        self.outcomes
            .iter()
            .filter_map(|g| crate::config::compile_glob(g).ok())
            .collect()
    }
    fn compares(&self, key: &str, outcomes: &[globset::GlobMatcher]) -> bool {
        !outcomes.iter().any(|g| g.is_match(key))
            && (self.compare == CompareMode::All || self.names(key))
    }
}
/// Apply map selection to already canonicalized metadata for ordinary strict reports.
pub(crate) fn select(meta: &mut crate::meta::Meta, map: &FingerprintMap) {
    let outcomes = map.outcome_globs();
    meta.retain(|key, _| map.compares(key, &outcomes));
}
/// Resolve explicit derivations from varied destination/source fields; ignores never activate them.
pub(crate) fn derived_keys(map: Option<&FingerprintMap>, vary: &[String]) -> BTreeSet<String> {
    let mut covered = BTreeSet::new();
    if let Some(map) = map {
        loop {
            let before = covered.len();
            for (key, source) in &map.fields {
                if !token_matches(vary, key, Some(map)).is_empty() || covered.contains(key) {
                    covered.extend(source.derives.iter().cloned());
                }
            }
            if covered.len() == before {
                break;
            }
        }
    }
    covered
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
        v = match v {
            Value::Array(items) if !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()) => {
                items.get(part.parse::<usize>().ok()?)?
            }
            Value::Object(object) => object.get(part)?,
            _ => return None,
        };
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
    if let Some(map) = map {
        for file in &map.record_files {
            if let Some(v) = raw(&safe_sibling(root, file)?)? {
                merge(&mut primary, v);
            }
        }
    }
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
            meta.insert("run.readiness".into(), Value::Array(map.readiness.iter().map(|r| {
                let mut record = serde_json::json!({"criterion":{"name":r.name,"parameters":r.parameters}});
                if let Some(v) = get(&r.reached) { record["reached"] = v; }
                if let Some(v) = get(&r.observed) { record["observed"] = v; }
                record
            }).collect()));
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
fn token_matches(tokens: &[String], key: &str, map: Option<&FingerprintMap>) -> Vec<TokenMatch> {
    let mut sources = BTreeSet::new();
    let mut add_source = |dest: &str, source: &Source| {
        let dest = dest.trim_end_matches("[]");
        if key == dest {
            sources.insert(source.path.clone());
        } else if let Some(suffix) = key.strip_prefix(dest).filter(|s| s.starts_with('.')) {
            sources.insert(format!("{}{suffix}", source.path));
        }
    };
    if let Some(map) = map {
        for (dest, source) in &map.fields {
            add_source(dest, source);
        }
        for r in &map.readiness {
            add_source(&format!("run.readiness.{}.reached", r.name), &r.reached);
            add_source(&format!("run.readiness.{}.observed", r.name), &r.observed);
        }
    }
    let mut result = Vec::new();
    for token in tokens {
        for (via, name) in std::iter::once((MatchedName::Destination, key))
            .chain(sources.iter().map(|s| (MatchedName::Source, s.as_str())))
        {
            if matches(token, name) {
                let matched = TokenMatch {
                    token: token.clone(),
                    via,
                    name: name.into(),
                };
                if !result.contains(&matched) {
                    result.push(matched);
                }
            }
        }
    }
    result
}
fn known(value: Option<&Value>) -> bool {
    value.is_some()
}
fn state(value: Option<&Value>) -> &'static str {
    match value {
        None => "missing",
        Some(Value::Null) => "null",
        Some(_) => "value",
    }
}
// Collapse only object-vs-null roots. Ordinary object fields remain independently variable.
fn null_objects(a: &mut crate::meta::Meta, b: &mut crate::meta::Meta) {
    let keys: BTreeSet<_> = a
        .iter()
        .chain(b.iter())
        .filter(|(_, v)| v.is_null())
        .map(|(k, _)| k.clone())
        .collect();
    for key in keys {
        let prefix = format!("{key}.");
        for m in [&mut *a, &mut *b] {
            if !m.contains_key(&key) && m.keys().any(|k| k.starts_with(&prefix)) {
                let mut object = serde_json::Map::new();
                for (k, v) in m.iter().filter(|(k, _)| k.starts_with(&prefix)) {
                    object.insert(k[prefix.len()..].into(), v.clone());
                }
                m.insert(key.clone(), Value::Object(object));
                m.retain(|k, _| !k.starts_with(&prefix));
            }
        }
    }
}
fn finding(key: &str, a: &crate::meta::Meta, b: &crate::meta::Meta, reason: &str) -> Finding {
    Finding {
        key: key.into(),
        baseline: a.get(key).cloned(),
        capture: b.get(key).cloned(),
        baseline_state: state(a.get(key)).into(),
        capture_state: state(b.get(key)).into(),
        reason: reason.into(),
        token_matches: Vec::new(),
    }
}
pub(crate) fn readiness(meta: &mut crate::meta::Meta) {
    let records = match meta.remove("run.readiness") {
        Some(Value::Array(records)) => records,
        None => return,
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
            if let Some(v) = v {
                meta.insert(format!("{key}.{field}"), v.clone());
            }
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
    let mut checked = check_mapped(a, b, vary, ignore, None, &[]);
    checked.finish();
    checked
}
fn check_mapped(
    a: &crate::meta::Meta,
    b: &crate::meta::Meta,
    vary: &[String],
    ignore: &[String],
    map: Option<&FingerprintMap>,
    allow_unreached: &[String],
) -> Check {
    let derived = derived_keys(map, vary);
    let mut covered_by_derivation = Vec::new();
    let mut covered_by_vary = Vec::new();
    let (mut a, mut b) = (a.clone(), b.clone());
    null_objects(&mut a, &mut b);
    let mut bad = Vec::new();
    let outcome_globs = map.map_or_else(Vec::new, FingerprintMap::outcome_globs);
    let compares = |key: &str| map.is_none_or(|m| m.compares(key, &outcome_globs));
    let mut unmapped = KeySummary::default();
    let mut outcomes = KeySummary::default();
    // Mapped-only requires every named destination even if absent on both arms.
    // All retains the historical native identity and union-of-present-keys checks.
    if let Some(map) = map
        && map.compare == CompareMode::MappedOnly
    {
        for dest in map.fields.keys() {
            let key = dest.trim_end_matches("[]");
            let present = |meta: &crate::meta::Meta| {
                meta.keys()
                    .any(|k| k == key || k.starts_with(&format!("{key}.")))
            };
            if compares(key) && (!present(&a) || !present(&b)) {
                bad.push(finding(key, &a, &b, "missing"));
            }
        }
    }
    for key in [
        "fingerprint.schema",
        "producer.binary",
        "inputs.identity",
        "run.mode",
        "run.session",
    ] {
        if !compares(key) {
            continue;
        }
        if !known(a.get(key)) || !known(b.get(key)) {
            bad.push(finding(key, &a, &b, "missing"));
        } else if !a[key].is_string()
            || !b[key].is_string()
            || a[key].as_str().is_some_and(|s| s.trim().is_empty())
            || b[key].as_str().is_some_and(|s| s.trim().is_empty())
            || (key == "fingerprint.schema"
                && (a[key] != FINGERPRINT_SCHEMA || b[key] != FINGERPRINT_SCHEMA))
        {
            bad.push(finding(key, &a, &b, "malformed"));
        }
    }
    for group in ["producer.build", "run.env"] {
        if !compares(group)
            && !map.is_some_and(|m| {
                m.fields
                    .keys()
                    .any(|k| k.starts_with(&format!("{group}.")) && compares(k))
            })
        {
            continue;
        }
        let present = |m: &crate::meta::Meta| {
            m.get(group).is_some_and(|v| {
                v.is_object()
                    && (group == "run.env" || v.as_object().is_some_and(|o| !o.is_empty()))
            }) || m
                .iter()
                .any(|(k, v)| k.starts_with(&format!("{group}.")) && known(Some(v)))
        };
        if !present(&a) || !present(&b) {
            bad.push(finding(
                group,
                &a,
                &b,
                if a.get(group).is_some_and(Value::is_null)
                    || b.get(group).is_some_and(Value::is_null)
                {
                    "malformed"
                } else {
                    "missing"
                },
            ));
        }
    }
    readiness(&mut a);
    readiness(&mut b);
    if (map.is_none_or(|m| {
        m.compare == CompareMode::All
            || !m.readiness.is_empty()
            || m.fields.contains_key("run.readiness[]")
    })) && (!a.keys().any(|k| k.starts_with("run.readiness."))
        || !b.keys().any(|k| k.starts_with("run.readiness.")))
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
    let mut allowed_unreached = Vec::new();
    let mut permitted = BTreeSet::new();
    permitted.extend(allow_unreached.iter().cloned());
    if let Some(map) = map {
        permitted.extend(
            map.readiness
                .iter()
                .filter(|r| r.unreached_policy == UnreachedPolicy::Matched)
                .map(|r| r.name.clone()),
        );
    }
    for name in &permitted {
        let prefix = format!("run.readiness.{name}");
        if a.get(&format!("{prefix}.reached")) == Some(&Value::Bool(false))
            && b.get(&format!("{prefix}.reached")) == Some(&Value::Bool(false))
            && a.get(&format!("{prefix}.criterion")) == b.get(&format!("{prefix}.criterion"))
            && let (Some(av), Some(bv)) = (
                a.get(&format!("{prefix}.observed")),
                b.get(&format!("{prefix}.observed")),
            )
            && av == bv
        {
            allowed_unreached.push(AllowedUnreached {
                criterion: name.clone(),
                baseline_observed: av.clone(),
                capture_observed: bv.clone(),
            });
        }
    }
    let keys: BTreeSet<_> = a.keys().chain(b.keys()).collect();
    let mut ignored = Vec::new();
    for key in keys {
        if let Some(map) = map {
            if outcome_globs.iter().any(|g| g.is_match(key)) {
                outcomes.keys.push(key.clone());
                continue;
            }
            if !map.names(key) {
                unmapped.keys.push(key.clone());
                if map.compare == CompareMode::MappedOnly {
                    continue;
                }
            }
        }
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
            && !allowed_unreached
                .iter()
                .any(|r| key == &format!("run.readiness.{}.reached", r.criterion))
        {
            Some("readiness_not_reached")
        } else if key.starts_with("run.readiness.")
            && key.ends_with(".observed")
            && permitted.contains(
                key.trim_start_matches("run.readiness.")
                    .trim_end_matches(".observed"),
            )
            && (a.get(&key.replace(".observed", ".reached")) == Some(&Value::Bool(false))
                || b.get(&key.replace(".observed", ".reached")) == Some(&Value::Bool(false)))
            && av != bv
        {
            Some("unreached_observation_mismatch")
        } else if av != bv {
            Some("difference")
        } else {
            None
        };
        let vary_matches = token_matches(vary, key, map);
        let ignore_matches = token_matches(ignore, key, map);
        if reason.is_none() && av == Some(&Value::Null) && !ignore_matches.is_empty() {
            let mut f = finding(key, &a, &b, "equal_null");
            f.token_matches = ignore_matches.clone();
            ignored.push(f);
        }
        if let Some(reason) = reason {
            let mut f = finding(key, &a, &b, reason);
            if reason == "difference"
                && key != "fingerprint.schema"
                && !(key.starts_with("run.readiness.") && key.ends_with(".criterion"))
                && vary_matches.is_empty()
                && derived.contains(key)
            {
                covered_by_derivation.push(finding(key, &a, &b, "covered_by_derivation"));
            } else if reason == "difference"
                && key != "fingerprint.schema"
                && key != "run.session"
                && !(key.starts_with("run.readiness.") && key.ends_with(".criterion"))
                && !ignore_matches.is_empty()
            {
                f.token_matches = ignore_matches;
                ignored.push(f);
            } else if reason != "difference"
                || key == "fingerprint.schema"
                || (key.starts_with("run.readiness.") && key.ends_with(".criterion"))
                || vary_matches.is_empty()
            {
                bad.push(f);
            } else {
                f.token_matches = vary_matches;
                covered_by_vary.push(f);
            }
        }
    }
    bad.retain(|f| {
        if !compares(&f.key) {
            return false;
        }
        if (f.reason == "missing"
            || (f.reason == "difference"
                && f.key != "fingerprint.schema"
                && f.key != "run.session"
                && !(f.key.starts_with("run.readiness.") && f.key.ends_with(".criterion")))
            || (f.reason == "malformed"
                && (f.baseline_state == "null" || f.capture_state == "null")))
            && !token_matches(ignore, &f.key, map).is_empty()
        {
            let mut f = f.clone();
            f.token_matches = token_matches(ignore, &f.key, map);
            ignored.push(f);
            false
        } else {
            true
        }
    });
    ignored.sort_by(|a, b| a.key.cmp(&b.key));
    ignored.dedup();
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
        compare: map.map_or(CompareMode::All, |m| m.compare),
        unmapped,
        outcomes,
        offending: bad,
        ignore: ignore.to_vec(),
        ignored,
        covered_by_vary,
        covered_by_derivation,
        allowed_unreached,
        diagnostics: Vec::new(),
        vary: vary.to_vec(),
    }
}
fn apply_compare(map: &mut Option<FingerprintMap>, compare: Option<CompareMode>) -> Result<()> {
    if let Some(mode) = compare {
        if let Some(map) = map {
            map.compare = mode;
        } else if mode == CompareMode::MappedOnly {
            return Err(Error::Config(
                "mapped_only requires a fingerprint map".into(),
            ));
        }
    }
    Ok(())
}
/// Load and check two captures using effective metadata settings.
pub fn check_paths(a: &Path, b: &Path, opts: &crate::meta::MetaOptions) -> Result<Check> {
    let mut map = opts
        .fingerprint_map
        .as_deref()
        .map(FingerprintMap::read)
        .transpose()?;
    apply_compare(&mut map, opts.compare)?;
    let mut checked = check_loaded(a, b, opts, map.as_ref())?;
    checked.finish();
    Ok(checked)
}
fn check_loaded(
    a: &Path,
    b: &Path,
    opts: &crate::meta::MetaOptions,
    map: Option<&FingerprintMap>,
) -> Result<Check> {
    Ok(check_mapped(
        &load(a, &opts.name, map)?,
        &load(b, &opts.name, map)?,
        &opts.intended,
        &opts.ignore,
        map,
        &opts.allow_unreached,
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
    let mut map = cfg
        .meta
        .fingerprint_map
        .as_deref()
        .map(FingerprintMap::read)
        .transpose()?;
    apply_compare(&mut map, cfg.meta.compare)?;
    let mut checks = Vec::new();
    let mut identity_found = [false; 2];
    let mut searched = BTreeSet::from([cfg.meta.name.clone()]);
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
            let am = load_named(a, key, &cfg.meta.name, map.as_ref())?;
            let bm = load_named(b, key, &cfg.meta.name, map.as_ref())?;
            for (i, m) in [&am, &bm].into_iter().enumerate() {
                identity_found[i] |= m
                    .keys()
                    .any(|k| k.starts_with("producer.") || k.starts_with("inputs."));
            }
            let image = Path::new(key);
            let mut parent = PathBuf::new();
            if let Some(dir) = image.parent() {
                for part in dir.components() {
                    parent.push(part);
                    searched.insert(crate::paths::portable(&parent.join(&cfg.meta.name)));
                }
            }
            let stem = image.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            searched.insert(crate::paths::portable(
                &parent.join(format!("{stem}.{}", cfg.meta.name)),
            ));
            let mut c = check_mapped(
                &am,
                &bm,
                &cfg.meta.intended,
                &cfg.meta.ignore,
                map.as_ref(),
                &cfg.meta.allow_unreached,
            );
            for f in c
                .offending
                .iter_mut()
                .chain(&mut c.ignored)
                .chain(&mut c.covered_by_vary)
                .chain(&mut c.covered_by_derivation)
            {
                f.key = format!("{key}:{}", f.key);
            }
            checks.push(c);
        }
        if checks.is_empty() {
            checks.push(check_loaded(a, b, &cfg.meta, map.as_ref())?);
        }
    } else {
        checks.push(check_loaded(a, b, &cfg.meta, map.as_ref())?);
    }
    let mut merged = checks.remove(0);
    for c in checks {
        merged.unmapped.keys.extend(c.unmapped.keys);
        merged.outcomes.keys.extend(c.outcomes.keys);
        merged.offending.extend(c.offending);
        merged.ignored.extend(c.ignored);
        merged.covered_by_vary.extend(c.covered_by_vary);
        merged.covered_by_derivation.extend(c.covered_by_derivation);
        merged.allowed_unreached.extend(c.allowed_unreached);
        merged.diagnostics.extend(c.diagnostics);
        merged.exit_code = merged.exit_code.max(c.exit_code);
    }
    if merged.exit_code != 0 {
        merged.result = ComparisonResult::InvalidComparison;
    }
    merged.offending.sort_by(|a, b| a.key.cmp(&b.key));
    merged.offending.dedup();
    merged.ignored.sort_by(|a, b| a.key.cmp(&b.key));
    merged.ignored.dedup();
    merged.covered_by_vary.sort_by(|a, b| a.key.cmp(&b.key));
    merged.covered_by_vary.dedup();
    merged
        .covered_by_derivation
        .sort_by(|a, b| a.key.cmp(&b.key));
    merged.covered_by_derivation.dedup();
    merged
        .allowed_unreached
        .sort_by(|a, b| a.criterion.cmp(&b.criterion));
    merged.allowed_unreached.dedup();
    if a.is_dir() && b.is_dir() {
        for (i, root) in [a, b].into_iter().enumerate() {
            let meta = load(root, &cfg.meta.name, map.as_ref())?;
            if !identity_found[i]
                && !meta
                    .keys()
                    .any(|k| k.starts_with("producer.") || k.starts_with("inputs."))
            {
                let mut files = searched.clone();
                if let Some(map) = &map {
                    files.extend(map.record_files.iter().cloned());
                    files.extend(map.fields.values().filter_map(|s| s.file.clone()));
                    files.extend(
                        map.readiness
                            .iter()
                            .flat_map(|r| [r.reached.file.clone(), r.observed.file.clone()])
                            .flatten(),
                    );
                }
                let total = files.len();
                let mut files = files.into_iter().take(64).collect::<Vec<_>>();
                if total > 64 {
                    files.push(format!("{} additional files", total - 64));
                }
                merged.diagnostics.push(format!("No fingerprint identity found in arm {}. Looked at: {}. Use --fingerprint-map FILE with record_files = [\"capture.json\", \"cost-card.json\"] or --meta-name NAME to select the run record.", crate::paths::portable(root), files.join(", ")));
            }
        }
    }
    merged.finish();
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
    fn missing_content_requires_an_explicit_ignore() {
        let mut a = arm();
        a.remove("inputs.identity");
        let c = check(&a, &a, &["inputs".into()], &[]);
        assert_eq!(c.exit_code, 4);
        assert_eq!(c.offending[0].key, "inputs.identity");
        assert_eq!(check(&a, &a, &[], &["inputs".into()]).exit_code, 0);
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
        assert_eq!(check(&a, &b, &[], &[]).exit_code, 4); // absence is missing unless explicitly ignored
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
    #[test]
    fn mapped_array_index_paths_and_unsupported_wildcards() {
        let tmp = tempfile::tempdir().unwrap();
        let primary = tmp.path().join("capture.json");
        let record = json!({"captures":[{"receiver":{"mode":"fixed","ready":true}}],"object":{"0":"numeric key"},"captures.0.receiver.mode":"flat override"});
        std::fs::write(&primary, serde_json::to_vec(&record).unwrap()).unwrap();
        let map_path = tmp.path().join("map.json");
        let mut map = json!({"compare":"mapped_only","fields":{"run.mode":{"path":"captures.0.receiver.mode"}},"readiness":[{"name":"receiver_ready","reached":{"path":"captures.0.receiver.ready"},"observed":{"path":"captures.0.receiver.ready"}}]});
        std::fs::write(&map_path, serde_json::to_vec(&map).unwrap()).unwrap();
        let loaded = FingerprintMap::read(&map_path).unwrap();
        let a = load(&primary, "saccade-meta.json", Some(&loaded)).unwrap();
        assert_eq!(a["run.mode"], json!("flat override"));
        assert_eq!(a["run.readiness"][0]["reached"], json!(true));
        assert_eq!(a["run.readiness"][0]["observed"], json!(true));
        assert_eq!(
            check_mapped(&a, &a, &[], &[], Some(&loaded), &[]).exit_code,
            0
        );
        assert_eq!(lookup(&record, "object.0"), Some(json!("numeric key")));
        for path in [
            "captures.1.receiver.ready",
            "captures.-1.receiver.ready",
            "captures.+0.receiver.ready",
            "captures.999999999999999999999999999999.receiver.ready",
        ] {
            assert_eq!(lookup(&record, path), None, "{path}");
        }
        for path in ["captures.*.receiver.ready", "captures.[*].receiver.ready"] {
            map["readiness"][0]["reached"]["path"] = json!(path);
            std::fs::write(&map_path, serde_json::to_vec(&map).unwrap()).unwrap();
            let error = FingerprintMap::read(&map_path).unwrap_err().to_string();
            assert!(
                error.contains(path) && error.contains("wildcards are unsupported"),
                "{error}"
            );
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod field_feedback_tests {
    use super::*;
    use serde_json::json;
    fn arm() -> crate::meta::Meta {
        let mut m = BTreeMap::new();
        flatten(
            "",
            &json!({"fingerprint":{"schema":FINGERPRINT_SCHEMA,"producer":{"binary":"sha256:b","build":{"profile":"debug"}},"inputs":{"identity":"sha256:i"},"run":{"mode":"fixed","env":{},"session":"one","readiness":[{"criterion":{"name":"warmup","parameters":{"limit":100}},"reached":true,"observed":12}]}},"content":{"hash":"sha256:c","preparation_report_sha256":"sha256:p","preparer":null}}),
            &mut m,
        );
        m
    }
    #[test]
    fn explicit_nulls_are_equal() {
        let a = arm();
        assert_eq!(check(&a, &a, &[], &[]).exit_code, 0);
    }
    #[test]
    fn null_versus_preparer_object_is_a_difference_and_can_be_varied() {
        let a = arm();
        let mut b = a.clone();
        b.remove("content.preparer");
        flatten(
            "content.preparer",
            &json!({"commit":"abc","dirty":false}),
            &mut b,
        );
        let c = check(&a, &b, &[], &[]);
        assert_eq!(c.exit_code, 3);
        assert_eq!(c.offending[0].baseline_state, "null");
        assert_eq!(c.offending[0].capture_state, "value");
        assert_eq!(check(&a, &b, &["preparer".into()], &[]).exit_code, 0);
    }
    #[test]
    fn absent_is_missing_and_explicit_ignores_retain_states() {
        let a = arm();
        let mut b = a.clone();
        b.remove("content.preparer");
        assert_eq!(check(&a, &b, &[], &[]).exit_code, 4);
        let c = check(&a, &b, &[], &["preparer".into()]);
        assert_eq!(c.exit_code, 0);
        assert_eq!(c.ignored[0].baseline_state, "null");
        assert_eq!(c.ignored[0].capture_state, "missing");
        assert_eq!(
            check(&a, &a, &[], &["preparer".into()]).ignored[0].reason,
            "equal_null"
        );
        b.insert("content.preparer".into(), json!("identity"));
        assert_eq!(check(&a, &b, &[], &["preparer".into()]).exit_code, 0);
        b.remove("inputs.identity");
        let c = check(&b, &b, &[], &["inputs.identity".into()]);
        assert_eq!(c.exit_code, 0);
        assert!(
            c.ignored
                .iter()
                .any(|f| f.key == "inputs.identity" && f.baseline_state == "missing")
        );
    }
    #[test]
    fn allow_unreached_requires_identical_observations_and_predicates() {
        let mut a = arm();
        a.get_mut("run.readiness").unwrap()[0]["reached"] = json!(false);
        let allowed = vec!["warmup".into()];
        assert_eq!(check(&a, &a, &[], &[]).exit_code, 3);
        let c = check_mapped(&a, &a, &[], &[], None, &allowed);
        assert_eq!(c.exit_code, 0);
        assert_eq!(c.allowed_unreached[0].baseline_observed, 12);
        assert_eq!(c.allowed_unreached[0].capture_observed, 12);
        let mut b = a.clone();
        b.get_mut("run.readiness").unwrap()[0]["observed"] = json!(13);
        assert_eq!(
            check_mapped(&a, &b, &["run.*".into()], &[], None, &allowed).exit_code,
            3
        );
        b = a.clone();
        b.get_mut("run.readiness").unwrap()[0]["reached"] = json!(true);
        assert_eq!(check_mapped(&a, &b, &[], &[], None, &allowed).exit_code, 3);
        b = a.clone();
        b.get_mut("run.readiness").unwrap()[0]["criterion"]["parameters"]["limit"] = json!(101);
        assert_eq!(check_mapped(&a, &b, &[], &[], None, &allowed).exit_code, 3);
    }
    #[test]
    fn directory_records_and_map_readiness_policy_are_used() {
        let tmp = tempfile::tempdir().unwrap();
        let a = tmp.path().join("a");
        let b = tmp.path().join("b");
        for root in [&a, &b] {
            std::fs::create_dir(root).unwrap();
            std::fs::write(root.join("frame.png"), b"not decoded").unwrap();
            std::fs::write(root.join("capture.json"), br#"{"exe":{"hash":"sha256:b","profile":"debug"},"content":{"hash":"sha256:c","preparer":null},"mode":"fixed","session":"one","env":{},"reached":false,"frame_index":12}"#).unwrap();
            std::fs::write(
                root.join("cost-card.json"),
                br#"{"report_hash":"sha256:p"}"#,
            )
            .unwrap();
        }
        let path = tmp.path().join("map.json");
        std::fs::write(&path, br#"{"record_files":["capture.json","cost-card.json"],"fields":{"producer.binary":{"path":"exe.hash"},"producer.build.profile":{"path":"exe.profile"},"inputs.identity":{"path":"content.hash"},"inputs.report_hash":{"path":"report_hash"},"inputs.preparer":{"path":"content.preparer"},"run.mode":{"path":"mode"},"run.session":{"path":"session"},"run.env":{"path":"env"}},"readiness":[{"name":"warmup","unreached_policy":"matched","reached":{"path":"reached"},"observed":{"path":"frame_index"}}]}"#).unwrap();
        let mut cfg = crate::config::RunConfig::default();
        let no_map = validate_paths(&a, &b, &cfg).unwrap();
        assert_eq!(no_map.exit_code, 4);
        assert!(no_map.diagnostics[0].contains("record_files"));
        assert!(no_map.diagnostics[0].contains("saccade-meta.json"));
        assert!(no_map.diagnostics[0].contains("frame.saccade-meta.json"));
        // Per-image records are a valid fingerprint source too: do not warn that
        // none were found merely because the root-level record is absent.
        for root in [&a, &b] {
            std::fs::write(
                root.join("frame.saccade-meta.json"),
                serde_json::to_vec(&arm()).unwrap(),
            )
            .unwrap();
        }
        let sidecars = validate_paths(&a, &b, &cfg).unwrap();
        assert_eq!(sidecars.exit_code, 0);
        assert!(sidecars.diagnostics.is_empty());
        for root in [&a, &b] {
            std::fs::remove_file(root.join("frame.saccade-meta.json")).unwrap();
        }
        cfg.meta.fingerprint_map = Some(path);
        let c = validate_paths(&a, &b, &cfg).unwrap();
        assert_eq!(c.exit_code, 0, "{c:?}");
        assert!(c.diagnostics.is_empty());
        assert_eq!(c.allowed_unreached.len(), 1);
        let map = FingerprintMap::read(cfg.meta.fingerprint_map.as_ref().unwrap()).unwrap();
        let loaded = load(&a, "saccade-meta.json", Some(&map)).unwrap();
        assert_eq!(loaded["inputs.preparer"], Value::Null);
        assert_eq!(loaded["inputs.report_hash"], "sha256:p");
        std::fs::remove_file(b.join("cost-card.json")).unwrap();
        assert_eq!(validate_paths(&a, &b, &cfg).unwrap().exit_code, 4);
    }
    fn selected_map(mode: CompareMode) -> FingerprintMap {
        let mut map: FingerprintMap = serde_json::from_value(
            json!({"fields":{"run.mode":{"path":"mode","derives":["cache_key"]}}}),
        )
        .unwrap();
        map.compare = mode;
        map
    }
    fn selected_pair(map: &FingerprintMap, a: Value, b: Value) -> Check {
        check_mapped(
            &mapped(Path::new("."), a, Some(map)).unwrap(),
            &mapped(Path::new("."), b, Some(map)).unwrap(),
            &[],
            &[],
            Some(map),
            &[],
        )
    }
    #[test]
    fn mapped_only_ignores_differing_unmapped_outcomes_and_lists_union() {
        let map = selected_map(CompareMode::MappedOnly);
        let mut c = selected_pair(
            &map,
            json!({"mode":"fixed","timing":1,"only_a":true}),
            json!({"mode":"fixed","timing":9,"only_b":true}),
        );
        c.finish();
        assert_eq!(c.exit_code, 0);
        assert_eq!(c.unmapped.keys, ["only_a", "only_b", "timing"]);
        assert_eq!(c.unmapped.count, 3);
        assert_eq!(serde_json::to_value(c).unwrap()["compare"], "mapped_only");
    }
    #[test]
    fn mapped_only_refuses_undeclared_mapped_difference() {
        let c = selected_pair(
            &selected_map(CompareMode::MappedOnly),
            json!({"mode":"fixed"}),
            json!({"mode":"alternate"}),
        );
        assert_eq!(c.exit_code, 3);
        assert_eq!(c.offending[0].key, "run.mode");
    }
    #[test]
    fn mapped_only_missing_mapped_field_refuses_even_if_missing_on_both() {
        let map = selected_map(CompareMode::MappedOnly);
        for b in [json!({}), json!({"mode":"fixed"})] {
            let c = selected_pair(&map, json!({}), b);
            assert_eq!(c.exit_code, 4);
            assert!(
                c.offending
                    .iter()
                    .any(|f| f.key == "run.mode" && f.reason == "missing")
            );
        }
    }
    #[test]
    fn all_mode_preserves_unmapped_difference_refusal() {
        let mut a = arm();
        let mut b = arm();
        a.insert("timing".into(), json!(1));
        b.insert("timing".into(), json!(2));
        let c = check_mapped(&a, &b, &[], &[], Some(&selected_map(CompareMode::All)), &[]);
        assert_eq!(c.exit_code, 3);
        assert!(c.offending.iter().any(|f| f.key == "timing"));
    }
    #[test]
    fn outcomes_globs_exclude_in_both_modes_including_missing_keys() {
        for mode in [CompareMode::All, CompareMode::MappedOnly] {
            let mut map = selected_map(mode);
            map.outcomes = vec!["timing.*".into()];
            let mut a = arm();
            let mut b = arm();
            a.insert("timing.gpu".into(), json!(1));
            b.insert("timing.gpu".into(), json!(2));
            b.insert("timing.cpu".into(), json!(3));
            let mut c = check_mapped(&a, &b, &[], &[], Some(&map), &[]);
            c.finish();
            assert_eq!(c.exit_code, 0);
            assert_eq!(c.outcomes.count, 2);
            assert_eq!(c.outcomes.keys, ["timing.cpu", "timing.gpu"]);
            assert!(!c.unmapped.keys.contains(&"timing.gpu".into()));
        }
    }
    #[test]
    fn mapped_tokens_report_destination_and_source_matches() {
        for compare in [CompareMode::All, CompareMode::MappedOnly] {
            let mut map = selected_map(compare);
            map.fields.insert(
                "inputs.identity".into(),
                Source {
                    file: None,
                    path: "content.hash".into(),
                    derives: vec!["cache_key".into()],
                },
            );
            let mut a = arm();
            // Canonicalization removes mapped record sources; use only effective fields here.
            a.retain(|key, _| !key.starts_with("content."));
            let mut b = a.clone();
            b.insert("inputs.identity".into(), json!("recooked"));
            assert_eq!(check_mapped(&a, &b, &[], &[], Some(&map), &[]).exit_code, 3);
            for (token, via, name) in [
                (
                    "inputs.identity",
                    MatchedName::Destination,
                    "inputs.identity",
                ),
                ("inputs", MatchedName::Destination, "inputs.identity"),
                ("inputs.*", MatchedName::Destination, "inputs.identity"),
                ("*.identity", MatchedName::Destination, "inputs.identity"),
                ("content.hash", MatchedName::Source, "content.hash"),
                ("content", MatchedName::Source, "content.hash"),
                ("content.*", MatchedName::Source, "content.hash"),
                ("*.hash", MatchedName::Source, "content.hash"),
                ("hash", MatchedName::Source, "content.hash"),
            ] {
                let expected = vec![TokenMatch {
                    token: token.into(),
                    via,
                    name: name.into(),
                }];
                for ignore in [false, true] {
                    let tokens = vec![token.into()];
                    let (vary, ignores) = if ignore {
                        (&[][..], tokens.as_slice())
                    } else {
                        (tokens.as_slice(), &[][..])
                    };
                    let c = check_mapped(&a, &b, vary, ignores, Some(&map), &[]);
                    assert_eq!(c.exit_code, 0, "{token} ignore={ignore}");
                    let fields = if ignore { c.ignored } else { c.covered_by_vary };
                    assert_eq!(fields.len(), 1);
                    assert_eq!(fields[0].key, "inputs.identity");
                    assert_eq!(fields[0].token_matches, expected);
                }
            }
            for token in ["contents", "*.hashes", "other.hash"] {
                assert_eq!(
                    check_mapped(&a, &b, &[token.into()], &[], Some(&map), &[]).exit_code,
                    3
                );
                assert_eq!(
                    check_mapped(&a, &b, &[], &[token.into()], Some(&map), &[]).exit_code,
                    3
                );
            }
            let c = check_mapped(&a, &b, &["*".into()], &[], Some(&map), &[]);
            assert_eq!(c.covered_by_vary[0].token_matches.len(), 2);
            let mut missing = b.clone();
            missing.remove("inputs.identity");
            assert_eq!(
                check_mapped(&a, &missing, &["content".into()], &[], Some(&map), &[]).exit_code,
                4
            );
            let c = check_mapped(&a, &missing, &[], &["content".into()], Some(&map), &[]);
            assert_eq!(c.exit_code, 0);
            assert!(
                c.ignored
                    .iter()
                    .all(|f| f.token_matches[0].via == MatchedName::Source)
            );
            let mut null = a.clone();
            null.insert("inputs.identity".into(), Value::Null);
            let c = check_mapped(&null, &null, &[], &["content".into()], Some(&map), &[]);
            assert_eq!(c.ignored[0].reason, "equal_null");
            assert_eq!(c.ignored[0].token_matches[0].name, "content.hash");
            assert!(derived_keys(Some(&map), &["content".into()]).contains("cache_key"));
            assert!(!derived_keys(Some(&map), &[]).contains("cache_key"));
            map.fields.insert(
                "run.env".into(),
                Source {
                    file: Some("setup.json".into()),
                    path: "environment".into(),
                    derives: vec![],
                },
            );
            let matched = token_matches(&["environment.mode".into()], "run.env.mode", Some(&map));
            assert_eq!(matched[0].name, "environment.mode");
            assert_eq!(matched[0].via, MatchedName::Source);
            map.readiness.push(ReadinessMap {
                name: "warmup".into(),
                parameters: BTreeMap::new(),
                unreached_policy: UnreachedPolicy::Refuse,
                reached: Source {
                    file: None,
                    path: "receiver.ready".into(),
                    derives: vec![],
                },
                observed: Source {
                    file: None,
                    path: "receiver.observed".into(),
                    derives: vec![],
                },
            });
            let mut unreached = a.clone();
            unreached.get_mut("run.readiness").unwrap()[0]["reached"] = json!(false);
            for ignore in [false, true] {
                let tokens = vec!["receiver".into()];
                let (vary, ignores) = if ignore {
                    (&[][..], tokens.as_slice())
                } else {
                    (tokens.as_slice(), &[][..])
                };
                let c = check_mapped(&a, &unreached, vary, ignores, Some(&map), &[]);
                assert_eq!(c.exit_code, 3);
                assert!(
                    c.offending
                        .iter()
                        .any(|f| f.reason == "readiness_not_reached")
                );
            }
        }
    }
    #[test]
    fn mapped_only_derives_are_compared_and_vary_activates_coverage() {
        let map = selected_map(CompareMode::MappedOnly);
        let a = mapped(
            Path::new("."),
            json!({"mode":"fixed","cache_key":"a"}),
            Some(&map),
        )
        .unwrap();
        let b = mapped(
            Path::new("."),
            json!({"mode":"alternate","cache_key":"b"}),
            Some(&map),
        )
        .unwrap();
        let c = check_mapped(&a, &b, &[], &[], Some(&map), &[]);
        assert_eq!(c.exit_code, 3);
        assert!(c.offending.iter().any(|f| f.key == "cache_key"));
        let c = check_mapped(&a, &b, &["mode".into()], &[], Some(&map), &[]);
        assert_eq!(c.exit_code, 0);
        assert_eq!(c.covered_by_derivation[0].key, "cache_key");
        assert_eq!(
            check_mapped(&a, &b, &[], &["mode".into()], Some(&map), &[]).exit_code,
            3
        );
    }
    #[test]
    fn excluded_summary_caps_keys_but_keeps_total() {
        let map = selected_map(CompareMode::MappedOnly);
        let mut a = arm();
        for i in 0..100 {
            a.insert(format!("outcome_{i:03}"), json!(i));
        }
        let mut c = check_mapped(&a, &a, &[], &[], Some(&map), &[]);
        c.finish();
        assert!(c.unmapped.count >= 100);
        assert_eq!(c.unmapped.keys.len(), 64);
        assert!(c.unmapped.truncated);
    }
}
