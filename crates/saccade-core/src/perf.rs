//! Engine-neutral run performance evidence. Scopes never enter additive totals.
//! Repeat floors use max minus min; unknown floors never imply zero noise.
#![allow(missing_docs)]

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

pub const DEFAULT_PERF_NAME: &str = "saccade-perf.json";
#[cfg(feature = "graphics")]
const QUANTUM_TOLERANCE_MS: f64 = 1e-6;
fn default_ticks() -> u32 {
    2
}
fn default_min_delta_ms() -> f64 {
    0.05
}
fn default_min_delta_pct() -> f64 {
    0.5
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, thiserror::Error)]
#[error("performance input {path}: {message}")]
pub struct PerfError {
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PerfOptions {
    pub name: String,
    pub noise: Option<PathBuf>,
    pub floor: Option<PerfNoise>,
    pub k: f64,
    pub resolution_ms: Option<f64>,
    pub resolution_ticks: Option<u32>,
    pub min_delta_ms: Option<f64>,
    pub min_delta_pct: Option<f64>,
    pub policy_sources: BTreeMap<String, String>,
}
impl Default for PerfOptions {
    fn default() -> Self {
        Self {
            name: DEFAULT_PERF_NAME.into(),
            noise: None,
            floor: None,
            k: 3.0,
            resolution_ms: None,
            resolution_ticks: None,
            min_delta_ms: None,
            min_delta_pct: None,
            policy_sources: BTreeMap::new(),
        }
    }
}
impl PerfOptions {
    pub fn validate(&self) -> crate::Result<()> {
        if self.name.is_empty()
            || self.name == "."
            || self.name == ".."
            || self.name.contains(['/', '\\', '\0', ':'])
        {
            return Err(crate::Error::Config(
                "perf_name must be a plain file name".into(),
            ));
        }
        if !self.k.is_finite() || self.k <= 0.0 {
            return Err(crate::Error::Config(
                "perf_noise_k must be finite and positive".into(),
            ));
        }
        if let Some(f) = &self.floor {
            f.validate()?;
        }
        self.policy(None).validate()?;
        Ok(())
    }
    /// Explicit options override calibration settings, then defaults apply.
    pub fn policy(&self, floor: Option<&PerfNoise>) -> PerfNoise {
        let mut policy = floor.cloned().unwrap_or_default();
        if let Some(v) = self.resolution_ms {
            policy.resolution_ms = Some(v);
        }
        if let Some(v) = self.resolution_ticks {
            policy.resolution_ticks = v;
        }
        if let Some(v) = self.min_delta_ms {
            policy.min_delta_ms = v;
        }
        if let Some(v) = self.min_delta_pct {
            policy.min_delta_pct = v;
        }
        policy
    }
    pub fn resolved_floor(&self) -> crate::Result<Option<PerfNoise>> {
        self.validate()?;
        let floor = match &self.noise {
            Some(p) => Some(PerfNoise::read(p)?),
            None => self.floor.clone(),
        };
        floor
            .map(|f| {
                let f = self.policy(Some(&f));
                f.validate()?;
                Ok(f)
            })
            .transpose()
    }
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Pass,
    Gap,
    Scope,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Frame {
    pub value: f64,
    pub samples: u64,
    pub stat: String,
}
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Spread {
    pub min: f64,
    pub max: f64,
}
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Term {
    pub id: String,
    #[serde(default)]
    pub label: Option<String>,
    pub kind: Kind,
    pub value: f64,
    #[serde(default)]
    pub parent: Option<String>,
    #[serde(default)]
    pub spread: Option<Spread>,
}
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapturePerf {
    pub schema: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    pub unit: String,
    pub frame: Frame,
    pub terms: Vec<Term>,
    pub counters: BTreeMap<String, BTreeMap<String, f64>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<PerfContext>,
}

// Historical input shape; its validator stays unchanged.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "schema", schemars(rename = "CapturePerf"))]
pub struct LegacyCapturePerf {
    pub schema: String,
    pub unit: String,
    pub frame: Frame,
    pub terms: Vec<Term>,
    pub counters: BTreeMap<String, BTreeMap<String, f64>>,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Comparability {
    Qualified,
    Rejected,
    #[default]
    Unknown,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceRef {
    pub source: String,
    pub hash: crate::evidence::canonical::Digest,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Qualification {
    pub status: Comparability,
    pub rule: String,
    pub version: String,
    /// Missing checks remain unknown; matching false checks reject both sides.
    pub checks: BTreeMap<String, Option<bool>>,
    pub reasons: Vec<String>,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnavailableTerm {
    pub id: String,
    pub parent: Option<String>,
    pub upper_bound_ms: Option<f64>,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PerfContext {
    pub measurement: Option<SourceRef>,
    pub timer: Option<String>,
    pub quantum_ms: Option<f64>,
    pub sample_window: Option<SourceRef>,
    pub raw_samples: Vec<SourceRef>,
    pub hardware: BTreeMap<String, String>,
    /// joint_partition means terms account for this frame's measured window.
    /// independent_statistics never supplies an additive frame remainder.
    pub aggregation: String,
    pub attribution_coverage: Option<f64>,
    pub unresolved_remainder_ms: Option<f64>,
    pub capture_hash: Option<crate::evidence::canonical::Digest>,
    pub configuration_hash: Option<crate::evidence::canonical::Digest>,
    pub producer: String,
    pub producer_version: String,
    pub qualification: Qualification,
    #[serde(default)]
    pub unavailable_terms: Vec<UnavailableTerm>,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PerformanceNoiseRecord {
    pub schema: String,
    pub kind: String,
    pub unit: String,
    pub perf_noise: PerfNoise,
    pub comparability: Comparability,
    pub sources: Vec<SourceRef>,
    pub reasons: Vec<String>,
    /// Declared binary and source identities for each measured repeat.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub capture_provenance: BTreeMap<String, BTreeMap<String, String>>,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PerfDocument {
    Measurement(Box<CapturePerf>),
    Noise(Box<PerformanceNoiseRecord>),
}
// serde maps otherwise silently overwrite duplicate counter names. Check every
// object before decoding into typed records, including nested counter maps.
struct JsonKeys;
impl<'de> Deserialize<'de> for JsonKeys {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = JsonKeys;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("JSON without duplicate keys")
            }
            fn visit_map<M: serde::de::MapAccess<'de>>(
                self,
                mut map: M,
            ) -> Result<JsonKeys, M::Error> {
                let mut keys = BTreeSet::new();
                while let Some(key) = map.next_key::<String>()? {
                    if !keys.insert(key.clone()) {
                        return Err(serde::de::Error::custom(format!("duplicate key {key:?}")));
                    }
                    map.next_value::<JsonKeys>()?;
                }
                Ok(JsonKeys)
            }
            fn visit_seq<S: serde::de::SeqAccess<'de>>(
                self,
                mut seq: S,
            ) -> Result<JsonKeys, S::Error> {
                while seq.next_element::<JsonKeys>()?.is_some() {}
                Ok(JsonKeys)
            }
            fn visit_bool<E: serde::de::Error>(self, _: bool) -> Result<JsonKeys, E> {
                Ok(JsonKeys)
            }
            fn visit_i64<E: serde::de::Error>(self, _: i64) -> Result<JsonKeys, E> {
                Ok(JsonKeys)
            }
            fn visit_u64<E: serde::de::Error>(self, _: u64) -> Result<JsonKeys, E> {
                Ok(JsonKeys)
            }
            fn visit_f64<E: serde::de::Error>(self, _: f64) -> Result<JsonKeys, E> {
                Ok(JsonKeys)
            }
            fn visit_str<E: serde::de::Error>(self, _: &str) -> Result<JsonKeys, E> {
                Ok(JsonKeys)
            }
            fn visit_unit<E: serde::de::Error>(self) -> Result<JsonKeys, E> {
                Ok(JsonKeys)
            }
        }
        d.deserialize_any(Visitor)
    }
}
fn nonnegative(v: f64) -> bool {
    v.is_finite() && v >= 0.0
}
impl CapturePerf {
    pub fn parse(text: &str, path: &str) -> Result<Self, PerfError> {
        let err = |message: String| PerfError {
            path: path.into(),
            message,
        };
        serde_json::from_str::<JsonKeys>(text).map_err(|e| err(e.to_string()))?;
        let value: serde_json::Value =
            serde_json::from_str(text).map_err(|e| err(e.to_string()))?;
        let schema = value.get("schema").and_then(|v| v.as_str()).unwrap_or("");
        if schema
            .strip_prefix("saccade-perf.v")
            .and_then(|v| v.parse::<u32>().ok())
            .is_some_and(|v| v > 2)
        {
            return Err(err(format!(
                "written by {schema}; installed saccade supports up to saccade-perf.v2, upgrade"
            )));
        }
        let p: Self = serde_json::from_value(value).map_err(|e| {
            if e.to_string().contains("unknown field") {
                err(format!("written by a newer producer; installed saccade supports up to saccade-perf.v2, upgrade: {e}"))
            } else {
                err(e.to_string())
            }
        })?;
        p.validate().map_err(err)?;
        Ok(p)
    }
    fn validate(&self) -> Result<(), String> {
        if !["saccade-perf.v1", "saccade-perf.v2"].contains(&self.schema.as_str())
            || self.unit != "ms"
        {
            return Err("expected schema saccade-perf.v1/v2 and unit ms".into());
        }
        if self.schema == "saccade-perf.v1" && self.context.is_some() {
            return Err("v1 cannot carry v2 qualification context".into());
        }
        if (self.schema == "saccade-perf.v2" && self.kind.as_deref() != Some("measurement"))
            || (self.schema == "saccade-perf.v1" && self.kind.is_some())
        {
            return Err("perf.v2 measurement needs kind measurement; v1 has no kind".into());
        }
        if !nonnegative(self.frame.value)
            || self.frame.samples == 0
            || !["p50", "p95", "p99", "mean", "min", "max"].contains(&self.frame.stat.as_str())
        {
            return Err("frame needs finite nonnegative value, positive samples and stat p50/p95/p99/mean/min/max".into());
        }
        let mut terms = BTreeMap::new();
        for t in &self.terms {
            if t.id.trim().is_empty() || !nonnegative(t.value) || terms.insert(&t.id, t).is_some() {
                return Err(format!("invalid or duplicate term id/value: {:?}", t.id));
            }
            if let Some(s) = &t.spread
                && (!nonnegative(s.min)
                    || !nonnegative(s.max)
                    || s.min > t.value
                    || s.max < t.value)
            {
                return Err(format!(
                    "invalid spread for {:?}: require min <= value <= max",
                    t.id
                ));
            }
            if (t.kind == Kind::Scope) != t.parent.is_some() {
                return Err(format!("only scope terms require a parent: {:?}", t.id));
            }
        }
        for t in &self.terms {
            let mut seen = BTreeSet::from([&t.id]);
            let mut parent = t.parent.as_ref();
            while let Some(id) = parent {
                let p = terms
                    .get(id)
                    .ok_or_else(|| format!("unknown parent {id:?}"))?;
                if !seen.insert(id) {
                    return Err(format!("parent cycle at {id:?}"));
                }
                if p.kind == Kind::Gap {
                    return Err(format!("scope parent {id:?} cannot be a gap"));
                }
                parent = p.parent.as_ref();
            }
        }
        if !self.additive().is_finite() {
            return Err("additive total overflows".into());
        }
        for (id, counters) in &self.counters {
            if !terms.contains_key(id) {
                return Err(format!("counters reference unknown term {id:?}"));
            }
            if counters
                .iter()
                .any(|(k, v)| k.trim().is_empty() || !v.is_finite())
            {
                return Err(format!("invalid counter for {id:?}"));
            }
        }
        if let Some(c) = &self.context {
            if !["joint_partition", "independent_statistics"].contains(&c.aggregation.as_str())
                || c.quantum_ms.is_some_and(|q| !q.is_finite() || q <= 0.0)
                || c.attribution_coverage
                    .is_some_and(|v| !nonnegative(v) || v > 1.0)
                || c.unresolved_remainder_ms.is_some_and(|v| !nonnegative(v))
                || c.measurement
                    .iter()
                    .chain(c.sample_window.iter())
                    .chain(&c.raw_samples)
                    .any(|s| s.source.trim().is_empty())
            {
                return Err("invalid performance context, aggregation or bound".into());
            }
            let mut parents: BTreeMap<&str, Option<&str>> = self
                .terms
                .iter()
                .map(|t| (t.id.as_str(), t.parent.as_deref()))
                .collect();
            for t in &c.unavailable_terms {
                if t.id.trim().is_empty()
                    || t.upper_bound_ms.is_some_and(|v| !nonnegative(v))
                    || parents.insert(&t.id, t.parent.as_deref()).is_some()
                {
                    return Err("invalid or duplicate unavailable term".into());
                }
            }
            for id in parents.keys() {
                let mut seen = BTreeSet::from([*id]);
                let mut parent = parents[id];
                while let Some(p) = parent {
                    if !seen.insert(p) {
                        return Err("unavailable term parent cycle".into());
                    }
                    parent = *parents.get(p).ok_or("unknown unavailable term parent")?;
                }
            }
        }
        Ok(())
    }
    pub fn qualification(&self) -> (Comparability, Vec<String>) {
        let Some(c) = &self.context else {
            return (
                Comparability::Unknown,
                vec!["qualification context missing (historical v1 or unavailable v2)".into()],
            );
        };
        let q = &c.qualification;
        if q.status == Comparability::Rejected || q.checks.values().any(|v| *v == Some(false)) {
            let mut reasons = q.reasons.clone();
            reasons.push("producer qualification rejected or a check failed".into());
            reasons.extend(
                q.checks
                    .iter()
                    .filter(|(_, v)| **v == Some(false))
                    .map(|(name, _)| format!("qualification check failed: {name}")),
            );
            return (Comparability::Rejected, reasons);
        }
        let complete = c.measurement.is_some()
            && c.sample_window.is_some()
            && !c.raw_samples.is_empty()
            && c.timer.as_ref().is_some_and(|s| !s.trim().is_empty())
            && c.quantum_ms.is_some()
            && c.capture_hash.is_some()
            && c.configuration_hash.is_some()
            && ["gpu", "driver"]
                .iter()
                .all(|k| c.hardware.get(*k).is_some_and(|v| !v.trim().is_empty()))
            && !c.producer.trim().is_empty()
            && !c.producer_version.trim().is_empty()
            && !q.rule.trim().is_empty()
            && !q.version.trim().is_empty()
            && ["window_complete", "clock_qualified", "warmup_complete"]
                .iter()
                .all(|name| q.checks.get(*name) == Some(&Some(true)))
            && q.checks.values().all(|v| *v == Some(true));
        if q.status != Comparability::Qualified || !complete || self.validate().is_err() {
            let mut reasons = q.reasons.clone();
            reasons.extend(
                q.checks
                    .iter()
                    .filter(|(_, v)| v.is_none())
                    .map(|(name, _)| format!("{name} evidence unavailable")),
            );
            if reasons.is_empty() {
                reasons.push(
                    "qualification or required source/timer/window/hardware identity missing"
                        .into(),
                );
            }
            return (Comparability::Unknown, reasons);
        }
        (Comparability::Qualified, Vec::new())
    }

    /// Calibration identity excludes capture/window identity, retaining pair conditions.
    pub fn comparison_identity(&self) -> Option<crate::evidence::canonical::Digest> {
        let c = self.context.as_ref()?;
        crate::evidence::canonical::digest(&serde_json::json!({
            "timer":c.timer,"quantum_ms":c.quantum_ms,"hardware":c.hardware,
            "configuration_hash":c.configuration_hash,"aggregation":c.aggregation,
            "stat":self.frame.stat,"rule":c.qualification.rule,"version":c.qualification.version,
            "checks":c.qualification.checks.keys().collect::<Vec<_>>()
        }))
        .ok()
    }

    #[cfg(feature = "graphics")]
    fn accounting_remainder(&self) -> Option<f64> {
        let c = self.context.as_ref()?;
        if c.aggregation == "joint_partition" {
            Some(self.remainder())
        } else {
            None
        }
    }
    pub fn additive(&self) -> f64 {
        self.terms
            .iter()
            .filter(|t| t.kind != Kind::Scope)
            .map(|t| t.value)
            .sum()
    }
    pub fn remainder(&self) -> f64 {
        self.frame.value - self.additive()
    }
    pub fn read(dir: &Path, name: &str) -> Result<Option<Self>, PerfError> {
        Self::read_with_source(dir, name).map(|value| value.map(|(capture, _)| capture))
    }
    fn read_with_source(dir: &Path, name: &str) -> Result<Option<(Self, SourceRef)>, PerfError> {
        let path = dir.join(name);
        let err = |message: String| PerfError {
            path: crate::paths::portable(&path),
            message,
        };
        let m = match std::fs::symlink_metadata(&path) {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(err(e.to_string())),
        };
        if !m.is_file() || m.file_type().is_symlink() || m.len() > 4 * 1024 * 1024 {
            return Err(err(
                "expected regular sidecar at most 4 MiB; symlinks are not followed".into(),
            ));
        }
        let text = std::fs::read_to_string(&path).map_err(|e| err(e.to_string()))?;
        let capture = Self::parse(&text, &crate::paths::portable(&path))?;
        Ok(Some((
            capture,
            SourceRef {
                source: crate::paths::portable(&path),
                hash: crate::evidence::canonical::Digest::of_bytes(text.as_bytes()),
            },
        )))
    }
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PerfNoise {
    pub frame: f64,
    #[serde(default)]
    pub terms: BTreeMap<String, f64>,
    /// Estimated timer quantum, or an explicit override, in ms. None means unknown.
    #[serde(default)]
    pub resolution_ms: Option<f64>,
    #[serde(default = "default_ticks")]
    pub resolution_ticks: u32,
    #[serde(default = "default_min_delta_ms")]
    pub min_delta_ms: f64,
    /// Percentage of the baseline frame, shared by frame and term thresholds.
    #[serde(default = "default_min_delta_pct")]
    pub min_delta_pct: f64,
    #[serde(default)]
    pub comparability: Comparability,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timer: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_identity: Option<crate::evidence::canonical::Digest>,
}
impl Default for PerfNoise {
    fn default() -> Self {
        Self {
            frame: 0.0,
            terms: BTreeMap::new(),
            resolution_ms: None,
            resolution_ticks: default_ticks(),
            min_delta_ms: default_min_delta_ms(),
            min_delta_pct: default_min_delta_pct(),
            comparability: Comparability::Unknown,
            timer: None,
            context_identity: None,
        }
    }
}
impl PerfNoise {
    pub fn validate(&self) -> crate::Result<()> {
        if !nonnegative(self.frame)
            || self
                .terms
                .iter()
                .any(|(k, v)| k.trim().is_empty() || !nonnegative(*v))
        {
            return Err(crate::Error::Config(
                "perf_noise values must be finite and nonnegative".into(),
            ));
        }
        if self
            .resolution_ms
            .is_some_and(|v| !v.is_finite() || v <= 0.0)
            || self.resolution_ticks == 0
            || !nonnegative(self.min_delta_ms)
            || !nonnegative(self.min_delta_pct)
            || self
                .resolution_ms
                .is_some_and(|v| !(v * f64::from(self.resolution_ticks)).is_finite())
        {
            return Err(crate::Error::Config(
                "perf_resolution_ms must be finite and positive; perf_resolution_ticks must be positive; perf_min_delta_ms and perf_min_delta_pct must be finite and nonnegative".into(),
            ));
        }
        Ok(())
    }
    pub fn read(path: &Path) -> crate::Result<Self> {
        let text = std::fs::read_to_string(path).map_err(crate::run::io_err(format!(
            "reading perf noise {}",
            path.display()
        )))?;
        let floor = if text.trim_start().starts_with('{') {
            serde_json::from_str::<JsonKeys>(&text)?;
            let v: serde_json::Value = serde_json::from_str(&text)?;
            if let Some(schema) = v.get("schema").and_then(|v| v.as_str())
                && schema
                    .strip_prefix("saccade-perf.v")
                    .and_then(|v| v.parse::<u32>().ok())
                    .is_some_and(|v| v > 2)
            {
                return Err(crate::Error::Config(format!(
                    "written by {schema}; installed saccade supports up to saccade-perf.v2, upgrade"
                )));
            }
            if v.get("kind")
                .and_then(|v| v.as_str())
                .is_some_and(|k| k != "performance_noise")
                || (v.get("schema").and_then(|v| v.as_str()) == Some("saccade-noise.v1")
                    && v.get("perf_noise").is_none())
            {
                return Err(wrong_noise_kind(
                    "performance",
                    "noise BASE REPEAT... --kind performance",
                ));
            }
            if v.get("schema").and_then(|v| v.as_str()) == Some("saccade-perf.v2") {
                if v.get("kind").and_then(|v| v.as_str()) != Some("performance_noise") {
                    return Err(wrong_noise_kind(
                        "performance",
                        "noise BASE REPEAT... --kind performance",
                    ));
                }
                let record: PerformanceNoiseRecord = serde_json::from_value(v).map_err(|e| {
                    if e.to_string().contains("unknown field") {
                        crate::Error::Config(format!("written by a newer producer; installed saccade supports up to saccade-perf.v2, upgrade: {e}"))
                    } else {
                        crate::Error::Json(e)
                    }
                })?;
                if record.kind != "performance_noise" || record.unit != "ms" {
                    return Err(wrong_noise_kind(
                        "performance",
                        "noise BASE REPEAT... --kind performance",
                    ));
                }
                let mut floor = record.perf_noise;
                // The outer record cannot promote unavailable/legacy calibration.
                if record.comparability != Comparability::Qualified || record.sources.len() < 2 {
                    floor.comparability = record.comparability;
                    if floor.comparability == Comparability::Qualified {
                        floor.comparability = Comparability::Unknown;
                    }
                }
                floor.validate()?;
                return Ok(floor);
            }
            serde_json::from_value::<Self>(v.get("perf_noise").cloned().ok_or_else(|| {
                wrong_noise_kind("performance", "noise BASE REPEAT... --kind performance")
            })?)?
        } else {
            let v: toml::Value =
                toml::from_str(&text).map_err(|e| crate::Error::Config(e.to_string()))?;
            if v.get("perf_noise").is_none()
                && (v.get("override").is_some() || v.get("threshold").is_some())
            {
                return Err(wrong_noise_kind(
                    "performance",
                    "noise BASE REPEAT... --kind performance",
                ));
            }
            #[derive(Deserialize)]
            struct Document {
                perf_noise: PerfNoise,
            }
            toml::from_str::<Document>(&text)
                .map_err(|e| crate::Error::Config(e.to_string()))?
                .perf_noise
        };
        floor.validate()?;
        Ok(floor)
    }
}

pub fn wrong_noise_kind(expected: &'static str, command: &'static str) -> crate::Error {
    crate::Error::WrongNoiseKind { expected, command }
}

/// Distinguishes dedicated noise artifacts from ordinary mixed image/perf configuration.
pub fn is_performance_noise_document(text: &str) -> bool {
    if text.trim_start().starts_with('{') {
        serde_json::from_str::<serde_json::Value>(text)
            .is_ok_and(|v| v.get("schema").and_then(|s| s.as_str()) == Some("saccade-perf.v2"))
    } else {
        toml::from_str::<toml::Value>(text).is_ok_and(|v| {
            v.as_table().is_some_and(|table| {
                table.get("perf_noise").is_some_and(toml::Value::is_table)
                    && table
                        .keys()
                        .all(|k| ["perf_noise", "perf_noise_k"].contains(&k.as_str()))
            })
        })
    }
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Delta {
    pub before: Option<f64>,
    pub after: Option<f64>,
    pub delta: Option<f64>,
    pub delta_pct: Option<f64>,
    /// Raw repeat range in ms.
    pub noise_floor: Option<f64>,
    /// Effective threshold: max(k × range, ticks × quantum, absolute/relative minimum).
    #[serde(default)]
    pub noise_threshold: Option<f64>,
    pub beyond_noise: Option<bool>,
    /// paired, appeared, disappeared, or not_comparable.
    pub status: String,
}
#[cfg(feature = "graphics")]
fn finite(v: f64) -> Option<f64> {
    v.is_finite().then_some(v)
}
impl Delta {
    #[cfg(feature = "graphics")]
    fn new(
        before: Option<f64>,
        after: Option<f64>,
        floor: Option<f64>,
        threshold: Option<f64>,
        _minimum: Option<f64>,
        comparable: bool,
    ) -> Self {
        let status = match (before, after, comparable) {
            (Some(_), Some(_), true) => "paired",
            (None, Some(_), _) => "appeared",
            (Some(_), None, _) => "disappeared",
            _ => "not_comparable",
        };
        let delta = if comparable {
            before.zip(after).and_then(|(b, a)| finite(a - b))
        } else {
            None
        };
        let delta_pct = delta
            .zip(before)
            .filter(|(_, b)| *b != 0.0)
            .and_then(|(d, b)| finite(d / b * 100.0));
        let beyond_noise = delta.and_then(|d| threshold.map(|limit| d.abs() > limit));
        Self {
            before,
            after,
            delta,
            delta_pct,
            noise_floor: floor,
            noise_threshold: threshold,
            beyond_noise,
            status: status.into(),
        }
    }
}
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CounterDiff {
    pub name: String,
    #[serde(flatten)]
    pub change: Delta,
}
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TermDiff {
    pub id: String,
    pub label: String,
    pub kind: Kind,
    pub parent: Option<String>,
    pub before_kind: Option<Kind>,
    pub after_kind: Option<Kind>,
    pub before_parent: Option<String>,
    pub after_parent: Option<String>,
    #[serde(flatten)]
    pub change: Delta,
    pub share_before: Option<f64>,
    pub share_after: Option<f64>,
    pub counters: Vec<CounterDiff>,
}
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PerfDiff {
    pub schema: String,
    pub unit: String,
    pub noise_k: f64,
    #[serde(default)]
    pub resolution_ms: Option<f64>,
    #[serde(default = "default_ticks")]
    pub resolution_ticks: u32,
    #[serde(default = "default_min_delta_ms")]
    pub min_delta_ms: f64,
    #[serde(default = "default_min_delta_pct")]
    pub min_delta_pct: f64,
    /// max(min_delta_ms, baseline frame × min_delta_pct / 100).
    #[serde(default)]
    pub minimum_delta_ms: f64,
    pub frame: Delta,
    pub unattributed_before: Option<f64>,
    pub unattributed_after: Option<f64>,
    pub terms: Vec<TermDiff>,
    pub warnings: Vec<String>,
    #[serde(default)]
    pub comparability: Comparability,
    #[serde(default)]
    pub noise_comparability: Comparability,
    #[serde(default)]
    pub qualification_reasons: Vec<String>,
    #[serde(default)]
    pub frame_change: FrameChange,
    #[serde(default)]
    pub attribution: Attribution,
    #[serde(default)]
    pub materiality: Materiality,
    /// Effective policy sources: explicit options, repeat calibration or defaults.
    #[serde(default)]
    pub policy_sources: BTreeMap<String, String>,
    /// Producer evidence is retained in analytical reports, including its hashes.
    #[serde(default)]
    pub context_before: Option<PerfContext>,
    #[serde(default)]
    pub context_after: Option<PerfContext>,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FrameChange {
    Faster,
    Slower,
    WithinMeasuredNoise,
    #[default]
    Unknown,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Attribution {
    Complete,
    Partial,
    Inconsistent,
    #[default]
    Unavailable,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Materiality {
    pub floor_ms: Option<f64>,
    pub unexplained_upper_bound_ms: Option<f64>,
    pub blocks_no_effect: bool,
    pub reasons: Vec<String>,
}
impl PerfDiff {
    #[cfg(feature = "graphics")]
    pub fn between(b: &CapturePerf, a: &CapturePerf, floor: Option<&PerfNoise>, k: f64) -> Self {
        Self::between_with_options(
            b,
            a,
            floor,
            &PerfOptions {
                k,
                ..Default::default()
            },
        )
    }
    #[cfg(feature = "graphics")]
    pub fn between_with_options(
        b: &CapturePerf,
        a: &CapturePerf,
        floor: Option<&PerfNoise>,
        opts: &PerfOptions,
    ) -> Self {
        let k = opts.k;
        let mut policy = opts.policy(floor);
        if policy.resolution_ms.is_none() {
            policy.resolution_ms = b
                .context
                .as_ref()
                .and_then(|c| c.quantum_ms)
                .zip(a.context.as_ref().and_then(|c| c.quantum_ms))
                .map(|(b, a)| b.max(a));
        }
        let minimum = policy
            .min_delta_ms
            .max(b.frame.value * (policy.min_delta_pct / 100.0))
            .min(f64::MAX);
        let hard_floor = policy
            .resolution_ms
            .and_then(|q| finite(minimum.max(q * f64::from(policy.resolution_ticks))));
        let threshold = |spread: Option<f64>| {
            spread
                .zip(hard_floor)
                .and_then(|(s, h)| finite((k * s).max(h)))
        };
        let (mut comparability, mut qualification_reasons) = pair_qualification(b, a);
        if opts.validate().is_err() || floor.is_some_and(|f| f.validate().is_err()) {
            comparability = Comparability::Rejected;
            qualification_reasons
                .push("invalid performance threshold policy or repeat calibration".into());
        }
        let bm: BTreeMap<_, _> = b.terms.iter().map(|t| (&t.id, t)).collect();
        let am: BTreeMap<_, _> = a.terms.iter().map(|t| (&t.id, t)).collect();
        let ids: BTreeSet<_> = bm.keys().chain(am.keys()).copied().collect();
        let mut terms = Vec::new();
        for id in ids {
            let bt = bm.get(id).copied();
            let at = am.get(id).copied();
            let Some(t) = at.or(bt) else { continue };
            let comparable = bt
                .zip(at)
                .is_some_and(|(b, a)| b.kind == a.kind && b.parent == a.parent);
            let empty = BTreeMap::new();
            let bc = b.counters.get(id).unwrap_or(&empty);
            let ac = a.counters.get(id).unwrap_or(&empty);
            let keys: BTreeSet<_> = bc.keys().chain(ac.keys()).collect();
            let mut counters: Vec<_> = keys
                .into_iter()
                .map(|name| CounterDiff {
                    name: name.clone(),
                    change: Delta::new(
                        bc.get(name).copied(),
                        ac.get(name).copied(),
                        None,
                        None,
                        None,
                        comparable,
                    ),
                })
                .collect();
            counters.sort_by(|a, b| {
                b.change
                    .delta_pct
                    .map(f64::abs)
                    .unwrap_or(-1.0)
                    .total_cmp(&a.change.delta_pct.map(f64::abs).unwrap_or(-1.0))
                    .then(a.name.cmp(&b.name))
            });
            terms.push(TermDiff {
                id: id.clone(),
                label: t.label.clone().unwrap_or_else(|| id.clone()),
                kind: t.kind,
                parent: t.parent.clone(),
                before_kind: bt.map(|t| t.kind),
                after_kind: at.map(|t| t.kind),
                before_parent: bt.and_then(|t| t.parent.clone()),
                after_parent: at.and_then(|t| t.parent.clone()),
                change: Delta::new(
                    bt.map(|t| t.value),
                    at.map(|t| t.value),
                    floor.and_then(|f| f.terms.get(id).copied()),
                    threshold(floor.and_then(|f| f.terms.get(id).copied())),
                    None,
                    comparable,
                ),
                share_before: bt
                    .filter(|_| b.frame.value > 0.0)
                    .and_then(|t| finite(t.value / b.frame.value)),
                share_after: at
                    .filter(|_| a.frame.value > 0.0)
                    .and_then(|t| finite(t.value / a.frame.value)),
                counters,
            });
        }
        let mut warnings = Vec::new();
        for (side, p) in [("before", b), ("after", a)] {
            if p.accounting_remainder()
                .is_some_and(|r| r.abs() > p.frame.value * 0.01)
            {
                warnings.push(format!("{side}: unattributed remainder {:.6} ms exceeds 1% of frame (negative means over-attribution)", p.remainder()));
            }
        }
        if b.frame.stat != a.frame.stat {
            warnings.push("frame statistics differ; frame delta is not comparable".into());
        }
        let materiality = assess_materiality(b, a, &terms, hard_floor);
        let attribution = if [b, a].iter().any(|p| {
            p.accounting_remainder()
                .is_some_and(|r| r < -QUANTUM_TOLERANCE_MS)
        }) {
            Attribution::Inconsistent
        } else if [b, a].iter().all(|p| {
            p.accounting_remainder()
                .is_some_and(|r| r.abs() <= QUANTUM_TOLERANCE_MS)
                && p.context.as_ref().is_some_and(|c| {
                    c.unavailable_terms.is_empty()
                        && c.attribution_coverage == Some(1.0)
                        && c.unresolved_remainder_ms == Some(0.0)
                })
        }) {
            Attribution::Complete
        } else if materiality.unexplained_upper_bound_ms.is_some() {
            Attribution::Partial
        } else {
            Attribution::Unavailable
        };
        let frame = Delta::new(
            Some(b.frame.value),
            Some(a.frame.value),
            floor.map(|f| f.frame),
            threshold(floor.map(|f| f.frame)),
            None,
            b.frame.stat == a.frame.stat,
        );
        let noise_comparability = floor.map_or(Comparability::Unknown, |f| {
            if f.comparability == Comparability::Qualified
                && f.timer.is_some()
                && b.context.as_ref().is_some_and(|c| c.timer == f.timer)
                && a.context.as_ref().is_some_and(|c| c.timer == f.timer)
                && f.context_identity.is_some()
                && b.comparison_identity() == f.context_identity
                && a.comparison_identity() == f.context_identity
            {
                Comparability::Qualified
            } else if f.comparability == Comparability::Rejected {
                Comparability::Rejected
            } else {
                Comparability::Unknown
            }
        });
        let frame_change = if comparability != Comparability::Qualified
            || noise_comparability != Comparability::Qualified
        {
            FrameChange::Unknown
        } else {
            match (frame.beyond_noise, frame.delta) {
                (Some(false), _) => FrameChange::WithinMeasuredNoise,
                (Some(true), Some(d)) if d < 0.0 => FrameChange::Faster,
                (Some(true), Some(_)) => FrameChange::Slower,
                _ => FrameChange::Unknown,
            }
        };
        let mut policy_sources = BTreeMap::new();
        for (key, explicit) in [
            ("resolution_ticks", opts.resolution_ticks.is_some()),
            ("min_delta_ms", opts.min_delta_ms.is_some()),
            ("min_delta_pct", opts.min_delta_pct.is_some()),
        ] {
            policy_sources.insert(
                key.into(),
                if explicit {
                    "explicit_options"
                } else if floor.is_some() {
                    "repeat_calibration"
                } else {
                    "default"
                }
                .into(),
            );
        }
        policy_sources.insert(
            "resolution_ms".into(),
            if opts.resolution_ms.is_some() {
                "explicit_options"
            } else if floor.is_some_and(|f| f.resolution_ms.is_some()) {
                "repeat_calibration"
            } else if policy.resolution_ms.is_some() {
                "measurement_timer"
            } else {
                "unknown"
            }
            .into(),
        );
        policy_sources.insert("noise_k".into(), "effective_options".into());
        policy_sources.extend(opts.policy_sources.clone());
        Self {
            schema: "saccade-perf-diff.v1".into(),
            unit: "ms".into(),
            noise_k: k,
            resolution_ms: policy.resolution_ms,
            resolution_ticks: policy.resolution_ticks,
            min_delta_ms: policy.min_delta_ms,
            min_delta_pct: policy.min_delta_pct,
            minimum_delta_ms: minimum,
            frame,
            unattributed_before: b.accounting_remainder(),
            unattributed_after: a.accounting_remainder(),
            terms,
            warnings,
            comparability,
            noise_comparability,
            qualification_reasons,
            frame_change,
            attribution,
            materiality,
            policy_sources,
            context_before: b.context.clone(),
            context_after: a.context.clone(),
        }
    }
    pub fn top(&self, n: usize, beyond_only: bool) -> Vec<&TermDiff> {
        let mut terms: Vec<_> = self
            .terms
            .iter()
            .filter(|t| {
                t.change.delta.is_some() && (!beyond_only || t.change.beyond_noise == Some(true))
            })
            .collect();
        terms.sort_by(|a, b| {
            b.change
                .delta
                .map(f64::abs)
                .unwrap_or(0.0)
                .total_cmp(&a.change.delta.map(f64::abs).unwrap_or(0.0))
                .then(a.id.cmp(&b.id))
        });
        terms.truncate(n);
        terms
    }
    pub fn flags(&self, identical: bool) -> (bool, bool) {
        let moved = self.frame.beyond_noise == Some(true)
            || self
                .terms
                .iter()
                .any(|t| t.change.beyond_noise == Some(true));
        let complete = self.resolution_ms.is_some()
            && self.frame.noise_floor.is_some()
            && self.frame.beyond_noise == Some(false)
            && self
                .terms
                .iter()
                .all(|t| t.change.status != "paired" || t.change.beyond_noise == Some(false));
        let qualified = self.comparability == Comparability::Qualified
            && self.noise_comparability == Comparability::Qualified;
        (
            identical
                && qualified
                && complete
                && !moved
                && self.materiality.floor_ms.is_some()
                && self.materiality.unexplained_upper_bound_ms.is_some()
                && !self.materiality.blocks_no_effect
                && self.attribution != Attribution::Inconsistent,
            identical && qualified && moved,
        )
    }
    /// Compact verdict prioritises established beyond-noise changes.
    pub fn verdict(&self) -> String {
        let top = self.top(1, true);
        let top = if top.is_empty() {
            self.top(1, false)
        } else {
            top
        };
        let mut parts: Vec<_> = top
            .into_iter()
            .map(|t| format!("{} {}", clean(&t.id), describe(&t.change, self.noise_k)))
            .collect();
        parts.push(format!("frame {}", describe(&self.frame, self.noise_k)));
        if self.comparability != Comparability::Qualified
            || self.noise_comparability != Comparability::Qualified
        {
            parts.push(format!(
                "performance {:?}; repeat qualification {:?}",
                self.comparability, self.noise_comparability
            ));
        }
        let incomplete = self
            .terms
            .iter()
            .filter(|t| t.change.status != "paired")
            .count();
        if incomplete > 0 {
            parts.push(format!("terms differ: {incomplete} not comparable"));
            parts.extend(
                self.not_comparable(3)
                    .into_iter()
                    .map(|t| self.describe_unpaired(t)),
            );
        }
        parts.join(" · ")
    }
    pub fn summary(&self, n: usize) -> String {
        let mut parts = Vec::new();
        for t in self.top(n, false) {
            parts.push(format!(
                "{} {}",
                clean(&t.id),
                describe(&t.change, self.noise_k)
            ));
            if !t.counters.is_empty() {
                parts.push(format!(
                    "{} counters: {}",
                    clean(&t.id),
                    t.counters
                        .iter()
                        .take(3)
                        .map(|c| format!(
                            "{} {}",
                            clean(&c.name),
                            if let Some(p) = c.change.delta_pct {
                                format!("{p:+.1}%")
                            } else {
                                format!("{} (not comparable relative move)", c.change.status)
                            }
                        ))
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
        }
        let unpaired: Vec<_> = self
            .not_comparable(3)
            .into_iter()
            .map(|t| self.describe_unpaired(t))
            .collect();
        parts.extend(unpaired);
        parts.push(format!("frame {}", describe(&self.frame, self.noise_k)));
        parts.join(" · ")
    }
    /// Unmatched or structurally different terms, largest recorded duration first.
    pub fn not_comparable(&self, n: usize) -> Vec<&TermDiff> {
        let mut terms: Vec<_> = self
            .terms
            .iter()
            .filter(|t| t.change.status != "paired")
            .collect();
        let magnitude = |t: &TermDiff| {
            t.change
                .before
                .unwrap_or(0.0)
                .max(t.change.after.unwrap_or(0.0))
        };
        terms.sort_by(|a, b| magnitude(b).total_cmp(&magnitude(a)).then(a.id.cmp(&b.id)));
        terms.truncate(n);
        terms
    }
    pub fn describe_unpaired(&self, t: &TermDiff) -> String {
        let value = t
            .change
            .before
            .unwrap_or(0.0)
            .max(t.change.after.unwrap_or(0.0));
        format!(
            "{} {} ({value:.4} ms; not comparable{})",
            clean(&t.id),
            t.change.status,
            if value > self.minimum_delta_ms {
                "; above minimum delta"
            } else {
                ""
            }
        )
    }
}
#[cfg(feature = "graphics")]
fn pair_qualification(b: &CapturePerf, a: &CapturePerf) -> (Comparability, Vec<String>) {
    let (bs, mut reasons) = b.qualification();
    let (as_, ar) = a.qualification();
    reasons.extend(ar);
    if bs == Comparability::Rejected || as_ == Comparability::Rejected {
        return (Comparability::Rejected, reasons);
    }
    if b.frame.stat != a.frame.stat {
        reasons.push("frame aggregation statistics differ".into());
        return (Comparability::Rejected, reasons);
    }
    if let Some((bc, ac)) = b.context.as_ref().zip(a.context.as_ref())
        && (bc.timer != ac.timer
            || bc.hardware != ac.hardware
            || bc.configuration_hash != ac.configuration_hash
            || bc.aggregation != ac.aggregation
            || bc.qualification.rule != ac.qualification.rule
            || bc.qualification.version != ac.qualification.version
            || bc
                .qualification
                .checks
                .keys()
                .ne(ac.qualification.checks.keys()))
    {
        if bc.configuration_hash != ac.configuration_hash {
            reasons.push("configuration hash mismatch".into());
        }
        if bc.timer != ac.timer {
            reasons.push("timer identity differs".into());
        }
        if bc.hardware != ac.hardware {
            reasons.push("hardware identity differs".into());
        }
        if bc.aggregation != ac.aggregation {
            reasons.push("aggregation differs".into());
        }
        if bc.qualification.rule != ac.qualification.rule
            || bc.qualification.version != ac.qualification.version
            || bc
                .qualification
                .checks
                .keys()
                .ne(ac.qualification.checks.keys())
        {
            reasons.push("qualification rules differ".into());
        }
        return (Comparability::Rejected, reasons);
    }
    if bs != Comparability::Qualified || as_ != Comparability::Qualified {
        (Comparability::Unknown, reasons)
    } else {
        (Comparability::Qualified, reasons)
    }
}

#[cfg(feature = "graphics")]
fn assess_materiality(
    b: &CapturePerf,
    a: &CapturePerf,
    terms: &[TermDiff],
    floor_ms: Option<f64>,
) -> Materiality {
    let unexplained: BTreeSet<_> = terms
        .iter()
        .filter(|t| t.change.status != "paired")
        .map(|t| t.id.as_str())
        .collect();
    fn side_bound(p: &CapturePerf, unexplained: &BTreeSet<&str>) -> Option<f64> {
        let c = p.context.as_ref()?;
        let mut nodes: BTreeMap<&str, (Option<&str>, Option<f64>)> = p
            .terms
            .iter()
            .filter(|t| unexplained.contains(t.id.as_str()))
            .map(|t| {
                (
                    t.id.as_str(),
                    (
                        t.parent.as_deref(),
                        Some(t.spread.as_ref().map_or(t.value, |s| s.max)),
                    ),
                )
            })
            .collect();
        for t in &c.unavailable_terms {
            nodes.insert(&t.id, (t.parent.as_deref(), t.upper_bound_ms));
        }
        // Find each selected node's closest selected ancestor, through resolved scopes too.
        let parents: BTreeMap<&str, Option<&str>> = p
            .terms
            .iter()
            .map(|t| (t.id.as_str(), t.parent.as_deref()))
            .chain(
                c.unavailable_terms
                    .iter()
                    .map(|t| (t.id.as_str(), t.parent.as_deref())),
            )
            .collect();
        let mut children: BTreeMap<Option<&str>, Vec<&str>> = BTreeMap::new();
        for (id, (parent, _)) in &nodes {
            let mut ancestor = *parent;
            while let Some(par) = ancestor {
                if nodes.contains_key(par) {
                    break;
                }
                ancestor = *parents.get(par)?;
            }
            children.entry(ancestor).or_default().push(id);
        }
        let mut pending: BTreeMap<&str, usize> = nodes
            .keys()
            .map(|id| (*id, children.get(&Some(*id)).map_or(0, Vec::len)))
            .collect();
        let mut ready: Vec<&str> = pending
            .iter()
            .filter(|(_, n)| **n == 0)
            .map(|(id, _)| *id)
            .collect();
        let mut nested: BTreeMap<&str, f64> = BTreeMap::new();
        let mut total = 0.0;
        while let Some(id) = ready.pop() {
            let own = nodes.get(id)?.1?;
            let value = finite(own.max(nested.get(id).copied().unwrap_or(0.0)))?;
            let mut ancestor = nodes.get(id)?.0;
            while let Some(par) = ancestor {
                if nodes.contains_key(par) {
                    break;
                }
                ancestor = *parents.get(par)?;
            }
            if let Some(parent) = ancestor {
                *nested.entry(parent).or_default() += value;
                let count = pending.get_mut(parent)?;
                *count -= 1;
                if *count == 0 {
                    ready.push(parent);
                }
            } else {
                total += value;
            }
        }
        let remainder = match p.accounting_remainder() {
            Some(r) => r.abs().max(c.unresolved_remainder_ms?),
            None => c.unresolved_remainder_ms?,
        };
        finite(total + remainder)
    }
    let upper = if b.validate().is_ok() && a.validate().is_ok() {
        side_bound(b, &unexplained)
            .zip(side_bound(a, &unexplained))
            .map(|(b, a)| b.max(a))
    } else {
        None
    };
    let mut reasons = Vec::new();
    if floor_ms.is_none() {
        reasons.push("materiality floor unavailable: missing frame/timer bound".into());
    }
    if upper.is_none() {
        reasons.push("unexplained term or frame remainder has no usable bound".into());
    }
    let blocks_no_effect = match upper.zip(floor_ms) {
        Some((u, f)) if u < f => false,
        Some((u, f)) => {
            reasons.push(format!(
                "unexplained aggregate {u:.6} ms is at least materiality floor {f:.6} ms"
            ));
            true
        }
        None => true,
    };
    Materiality {
        floor_ms,
        unexplained_upper_bound_ms: upper,
        blocks_no_effect,
        reasons,
    }
}

pub fn clean(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect()
}
fn describe(d: &Delta, k: f64) -> String {
    let Some(v) = d.delta else {
        return "not comparable".into();
    };
    let pct = d
        .delta_pct
        .map_or(String::new(), |p| format!(" ({p:+.1}%)"));
    let noise = match (d.beyond_noise, d.noise_threshold) {
        (Some(b), Some(f)) => format!(
            " ({} noise; threshold {:.4} ms; k={k})",
            if b { "beyond" } else { "within" },
            f
        ),
        (Some(false), None) => " (within minimum delta; repeat noise unknown)".into(),
        _ => " (noise unknown)".into(),
    };
    format!("{v:+.4} ms{pct}{noise}")
}

/// Diff once per run; callers record malformed inputs as error entries.
#[cfg(feature = "graphics")]
pub fn pair(
    before: &Path,
    after: &Path,
    opts: &PerfOptions,
) -> crate::Result<(Option<PerfDiff>, Vec<PerfError>)> {
    let floor = opts.resolved_floor()?;
    let (b, a) = (
        CapturePerf::read(before, &opts.name),
        CapturePerf::read(after, &opts.name),
    );
    let errors = [&b, &a]
        .into_iter()
        .filter_map(|r| r.as_ref().err().cloned())
        .collect();
    let diff = match (b, a) {
        (Ok(Some(b)), Ok(Some(a))) => {
            Some(PerfDiff::between_with_options(&b, &a, floor.as_ref(), opts))
        }
        _ => None,
    };
    Ok((diff, errors))
}

/// Repeat range for keys present with the same kind and parent in every capture.
#[cfg(feature = "graphics")]
pub fn noise(dirs: &[PathBuf], name: &str) -> crate::Result<(Option<PerfNoise>, Vec<String>)> {
    noise_with_options(
        dirs,
        &PerfOptions {
            name: name.into(),
            ..Default::default()
        },
    )
}
#[cfg(feature = "graphics")]
pub fn noise_with_options(
    dirs: &[PathBuf],
    opts: &PerfOptions,
) -> crate::Result<(Option<PerfNoise>, Vec<String>)> {
    opts.validate()?;
    let captures: Vec<_> = dirs
        .iter()
        .map(|d| CapturePerf::read(d, &opts.name).map_err(crate::Error::Perf))
        .collect::<crate::Result<_>>()?;
    noise_from_captures(&captures, opts)
}

#[cfg(feature = "graphics")]
fn noise_from_captures(
    captures: &[Option<CapturePerf>],
    opts: &PerfOptions,
) -> crate::Result<(Option<PerfNoise>, Vec<String>)> {
    let present: Vec<_> = captures.iter().flatten().collect();
    if present.is_empty() {
        return Ok((None, Vec::new()));
    }
    if present.len() != captures.len() {
        return Err(crate::Error::Config(
            "perf noise needs a sidecar in every repeat when any repeat has one".into(),
        ));
    }
    let Some(first) = present.first() else {
        return Ok((None, Vec::new()));
    };
    let hashes: Vec<_> = present
        .iter()
        .filter_map(|p| p.context.as_ref()?.capture_hash.as_ref())
        .collect();
    if hashes.len() == present.len() && hashes.iter().collect::<BTreeSet<_>>().len() != hashes.len()
    {
        return Err(crate::Error::Config("same capture, not a repeat".into()));
    }
    if present.iter().any(|p| p.frame.stat != first.frame.stat) {
        return Err(crate::Error::Config(
            "perf noise frame statistics must match".into(),
        ));
    }
    let range = |values: Vec<f64>| {
        values.iter().copied().fold(f64::NEG_INFINITY, f64::max)
            - values.iter().copied().fold(f64::INFINITY, f64::min)
    };
    let mut terms = BTreeMap::new();
    let mut warnings = Vec::new();
    let ids: BTreeSet<_> = present
        .iter()
        .flat_map(|p| p.terms.iter().map(|t| &t.id))
        .collect();
    for id in ids {
        let found: Vec<_> = present
            .iter()
            .filter_map(|p| p.terms.iter().find(|t| &t.id == id))
            .collect();
        if found.len() == present.len()
            && found
                .iter()
                .all(|t| t.kind == found[0].kind && t.parent == found[0].parent)
        {
            terms.insert(id.clone(), range(found.iter().map(|t| t.value).collect()));
        } else {
            warnings.push(format!(
                "perf noise: {} not comparable across all repeats",
                clean(id)
            ));
        }
    }
    let mut calibration = opts.policy(opts.resolved_floor()?.as_ref());
    calibration.frame = range(present.iter().map(|p| p.frame.value).collect());
    calibration.terms = terms;
    if calibration.resolution_ms.is_none() {
        calibration.resolution_ms = present
            .iter()
            .filter_map(|p| p.context.as_ref().and_then(|c| c.quantum_ms))
            .max_by(f64::total_cmp)
            .or_else(|| {
                estimate_quantum(
                    present
                        .iter()
                        .flat_map(|p| p.terms.iter().map(|t| t.value))
                        .collect(),
                )
            });
    }
    calibration.comparability = Comparability::Qualified;
    calibration.timer = first.context.as_ref().and_then(|c| c.timer.clone());
    calibration.context_identity = first.comparison_identity();
    if captures.len() < 2 {
        calibration.comparability = Comparability::Unknown;
        warnings.push("repeat noise needs at least two captures".into());
    }
    for p in &present {
        let (status, reasons) = pair_qualification(first, p);
        if status == Comparability::Rejected {
            calibration.comparability = Comparability::Rejected;
        } else if status == Comparability::Unknown
            && calibration.comparability != Comparability::Rejected
        {
            calibration.comparability = Comparability::Unknown;
        }
        warnings.extend(reasons);
    }
    let windows: BTreeSet<_> = present
        .iter()
        .filter_map(|p| p.context.as_ref()?.sample_window.as_ref().map(|s| &s.hash))
        .collect();
    if calibration.comparability == Comparability::Qualified && windows.len() != captures.len() {
        calibration.comparability = Comparability::Unknown;
        warnings.push("unchanged repeats require distinct measured sample windows".into());
    }
    calibration.validate()?;
    Ok((Some(calibration), warnings))
}

/// Typed performance noise output. Sources hash the exact sidecars used above.
#[cfg(feature = "graphics")]
pub fn noise_record(dirs: &[PathBuf], opts: &PerfOptions) -> crate::Result<PerformanceNoiseRecord> {
    opts.validate()?;
    let mut captures = Vec::new();
    let mut sources = Vec::new();
    for dir in dirs {
        let (capture, source) =
            CapturePerf::read_with_source(dir, &opts.name)?.ok_or_else(|| {
                crate::Error::Config(
                    "performance noise requires a performance sidecar in every repeat".into(),
                )
            })?;
        captures.push(Some(capture));
        sources.push(source);
    }
    let (floor, mut reasons) = noise_from_captures(&captures, opts)?;
    let floor = floor.ok_or_else(|| {
        crate::Error::Config("performance noise needs at least two captures".into())
    })?;
    let mut capture_provenance = BTreeMap::new();
    for (index, dir) in dirs.iter().enumerate() {
        let evidence =
            crate::meta::capture_evidence(dir, "capture.png", crate::meta::DEFAULT_META_NAME);
        for field in ["binary_sha256", "source_head"] {
            if !evidence.contains_key(field) {
                reasons.push(format!("run {index} {field} provenance is absent"));
            }
        }
        capture_provenance.insert(index.to_string(), evidence);
    }
    Ok(PerformanceNoiseRecord {
        schema: "saccade-perf.v2".into(),
        kind: "performance_noise".into(),
        unit: "ms".into(),
        comparability: floor.comparability,
        perf_noise: floor,
        sources,
        reasons,
        capture_provenance,
    })
}

// Adjacent distinct values contain the same GCD as all pairwise differences.
// Approximate Euclid tolerates decimal/floating roundoff below 1e-6 ms.
#[cfg(feature = "graphics")]
fn estimate_quantum(mut values: Vec<f64>) -> Option<f64> {
    values.sort_by(f64::total_cmp);
    let mut previous = None;
    let mut quantum = None;
    for value in values {
        let Some(prev) = previous else {
            previous = Some(value);
            continue;
        };
        let difference: f64 = value - prev;
        if difference < QUANTUM_TOLERANCE_MS {
            continue;
        }
        previous = Some(value);
        let Some(q) = quantum else {
            quantum = Some(difference);
            continue;
        };
        let mut large = difference.max(q);
        let mut small = difference.min(q);
        loop {
            let remainder = large % small;
            if remainder < QUANTUM_TOLERANCE_MS || small - remainder < QUANTUM_TOLERANCE_MS {
                break;
            }
            large = small;
            small = remainder;
        }
        quantum = Some(small);
    }
    quantum
}

/// Returns `feature_unavailable` when graphics computation is not compiled.
#[cfg(not(feature = "graphics"))]
pub fn pair(
    _before: &Path,
    _after: &Path,
    _opts: &PerfOptions,
) -> crate::Result<(Option<PerfDiff>, Vec<PerfError>)> {
    Err(crate::Error::FeatureUnavailable {
        feature: "graphics",
    })
}
/// Returns `feature_unavailable` when graphics computation is not compiled.
#[cfg(not(feature = "graphics"))]
pub fn noise_with_options(
    _dirs: &[PathBuf],
    _opts: &PerfOptions,
) -> crate::Result<(Option<PerfNoise>, Vec<String>)> {
    Err(crate::Error::FeatureUnavailable {
        feature: "graphics",
    })
}
/// Returns `feature_unavailable` when graphics computation is not compiled.
#[cfg(not(feature = "graphics"))]
pub fn noise(_dirs: &[PathBuf], _name: &str) -> crate::Result<(Option<PerfNoise>, Vec<String>)> {
    Err(crate::Error::FeatureUnavailable {
        feature: "graphics",
    })
}
