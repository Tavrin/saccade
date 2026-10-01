//! Engine-neutral run performance evidence. Scopes never enter additive totals.
//! Repeat floors use max minus min; unknown floors never imply zero noise.
#![allow(missing_docs)]

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

pub const DEFAULT_PERF_NAME: &str = "saccade-perf.json";

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
}
impl Default for PerfOptions {
    fn default() -> Self {
        Self {
            name: DEFAULT_PERF_NAME.into(),
            noise: None,
            floor: None,
            k: 3.0,
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
        Ok(())
    }
    pub fn resolved_floor(&self) -> crate::Result<Option<PerfNoise>> {
        self.validate()?;
        match &self.noise {
            Some(p) => Ok(Some(PerfNoise::read(p)?)),
            None => Ok(self.floor.clone()),
        }
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
    pub unit: String,
    pub frame: Frame,
    pub terms: Vec<Term>,
    pub counters: BTreeMap<String, BTreeMap<String, f64>>,
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
        let p: Self = serde_json::from_str(text).map_err(|e| err(e.to_string()))?;
        p.validate().map_err(err)?;
        Ok(p)
    }
    fn validate(&self) -> Result<(), String> {
        if self.schema != "saccade-perf.v1" || self.unit != "ms" {
            return Err("expected schema saccade-perf.v1 and unit ms".into());
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
        Ok(())
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
        Self::parse(&text, &crate::paths::portable(&path)).map(Some)
    }
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PerfNoise {
    pub frame: f64,
    #[serde(default)]
    pub terms: BTreeMap<String, f64>,
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
        Ok(())
    }
    pub fn read(path: &Path) -> crate::Result<Self> {
        let text = std::fs::read_to_string(path).map_err(crate::run::io_err(format!(
            "reading perf noise {}",
            path.display()
        )))?;
        let floor = if text.trim_start().starts_with('{') {
            let v: serde_json::Value = serde_json::from_str(&text)?;
            serde_json::from_value::<Self>(
                v.get("perf_noise")
                    .cloned()
                    .ok_or_else(|| crate::Error::Config("noise JSON needs perf_noise".into()))?,
            )?
        } else {
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

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Delta {
    pub before: Option<f64>,
    pub after: Option<f64>,
    pub delta: Option<f64>,
    pub delta_pct: Option<f64>,
    /// Raw repeat range in ms (threshold is k times this).
    pub noise_floor: Option<f64>,
    pub beyond_noise: Option<bool>,
    /// paired, appeared, disappeared, or not_comparable.
    pub status: String,
}
fn finite(v: f64) -> Option<f64> {
    v.is_finite().then_some(v)
}
impl Delta {
    fn new(
        before: Option<f64>,
        after: Option<f64>,
        floor: Option<f64>,
        k: f64,
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
        let beyond_noise = delta
            .zip(floor)
            .and_then(|(d, f)| finite(k * f).map(|limit| d.abs() > limit));
        Self {
            before,
            after,
            delta,
            delta_pct,
            noise_floor: floor,
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
    pub frame: Delta,
    pub unattributed_before: f64,
    pub unattributed_after: f64,
    pub terms: Vec<TermDiff>,
    pub warnings: Vec<String>,
}
impl PerfDiff {
    pub fn between(b: &CapturePerf, a: &CapturePerf, floor: Option<&PerfNoise>, k: f64) -> Self {
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
                        k,
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
                    k,
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
            if p.remainder().abs() > p.frame.value * 0.01 {
                warnings.push(format!("{side}: unattributed remainder {:.6} ms exceeds 1% of frame (negative means over-attribution)", p.remainder()));
            }
        }
        if b.frame.stat != a.frame.stat {
            warnings.push("frame statistics differ; frame delta is not comparable".into());
        }
        Self {
            schema: "saccade-perf-diff.v1".into(),
            unit: "ms".into(),
            noise_k: k,
            frame: Delta::new(
                Some(b.frame.value),
                Some(a.frame.value),
                floor.map(|f| f.frame),
                k,
                b.frame.stat == a.frame.stat,
            ),
            unattributed_before: b.remainder(),
            unattributed_after: a.remainder(),
            terms,
            warnings,
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
        let moved = self
            .terms
            .iter()
            .any(|t| t.change.beyond_noise == Some(true));
        let complete = !self.terms.is_empty()
            && self
                .terms
                .iter()
                .all(|t| t.change.status == "paired" && t.change.beyond_noise.is_some());
        (identical && complete && !moved, identical && moved)
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
        let incomplete = self
            .terms
            .iter()
            .filter(|t| t.change.status != "paired")
            .count();
        if incomplete > 0 {
            parts.push(format!("{incomplete} terms not comparable"));
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
            .terms
            .iter()
            .filter(|t| t.change.status != "paired")
            .take(3)
            .map(|t| format!("{} {} (not comparable)", clean(&t.id), t.change.status))
            .collect();
        parts.extend(unpaired);
        parts.push(format!("frame {}", describe(&self.frame, self.noise_k)));
        parts.join(" · ")
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
    let noise = match (d.beyond_noise, d.noise_floor) {
        (Some(b), Some(f)) => format!(
            " ({} noise {:.4} ms; k={k})",
            if b { "beyond" } else { "within" },
            f
        ),
        _ => " (noise unknown)".into(),
    };
    format!("{v:+.4} ms{pct}{noise}")
}

/// Diff once per run; callers record malformed inputs as error entries.
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
        (Ok(Some(b)), Ok(Some(a))) => Some(PerfDiff::between(&b, &a, floor.as_ref(), opts.k)),
        _ => None,
    };
    Ok((diff, errors))
}

/// Repeat range for keys present with the same kind and parent in every capture.
pub fn noise(dirs: &[PathBuf], name: &str) -> crate::Result<(Option<PerfNoise>, Vec<String>)> {
    let captures: Vec<_> = dirs
        .iter()
        .map(|d| CapturePerf::read(d, name).map_err(crate::Error::Perf))
        .collect::<crate::Result<_>>()?;
    let present: Vec<_> = captures.iter().flatten().collect();
    if present.is_empty() {
        return Ok((None, Vec::new()));
    }
    if present.len() != dirs.len() {
        return Err(crate::Error::Config(
            "perf noise needs a sidecar in every repeat when any repeat has one".into(),
        ));
    }
    let Some(first) = present.first() else {
        return Ok((None, Vec::new()));
    };
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
    Ok((
        Some(PerfNoise {
            frame: range(present.iter().map(|p| p.frame.value).collect()),
            terms,
        }),
        warnings,
    ))
}
