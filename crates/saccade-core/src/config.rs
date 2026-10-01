//! Run configuration and the `saccade.toml` file format.

use std::path::Path;

use globset::{Glob, GlobBuilder};
use serde::Deserialize;

use crate::error::{Error, Result};
use crate::report::{Labels, Metric, Mode};

/// A per-path override of the default threshold and/or metric.
#[derive(Debug, Clone, PartialEq)]
pub struct Override {
    /// Glob matched, case-insensitively, against the `/`-separated image name.
    /// `*` does not cross `/`; `**` does.
    pub glob: String,
    /// Replacement threshold, if any.
    pub threshold: Option<f64>,
    /// Replacement metric, if any.
    pub metric: Option<Metric>,
}

/// Settings for [`crate::run::run`].
#[derive(Debug, Clone, PartialEq)]
pub struct RunConfig {
    /// Default pass threshold.
    pub default_threshold: f64,
    /// Default deciding metric.
    pub default_metric: Metric,
    /// FLIP observer setting (pixels per degree).
    pub pixels_per_degree: f32,
    /// Whether a `new` entry counts as a regression.
    pub fail_on_new: bool,
    /// Globs of image names to skip entirely.
    pub ignore: Vec<String>,
    /// Restrict comparison to these name globs (empty selects all).
    pub entries: Vec<String>,
    /// Record absolute paths instead of portable relative paths.
    pub record_absolute_paths: bool,
    /// Per-path overrides; the first matching one wins, field by field.
    pub overrides: Vec<Override>,
    /// Numerical G-buffer comparison rules (`[[buffer]]`), first match wins.
    pub buffers: Vec<crate::buffer::BufferSpec>,
    /// Named regions of interest (`[[region]]`).
    pub regions: Vec<crate::regions::RegionSpec>,
    /// Excluded areas (`[[mask]]`).
    pub masks: Vec<crate::regions::MaskSpec>,
    /// Directory mask-image paths are relative to (the config file's directory).
    pub config_dir: Option<std::path::PathBuf>,
    /// What the run is for (`compare` or `identity`).
    pub mode: Mode,
    /// Display names of the two sides.
    pub labels: Labels,
    /// HDR-FLIP settings (`[hdr]`) for `.exr`/`.hdr` images.
    pub hdr: crate::hdr::HdrConfig,
    /// Metadata-sidecar settings (`meta_name` in the file, flags on the CLI).
    pub meta: crate::meta::MetaOptions,
    /// Run-level attribution and repeat noise settings.
    pub perf: crate::perf::PerfOptions,
    /// Error value above which a pixel belongs to a hotspot (`hotspot_threshold`).
    pub hotspot_threshold: f32,
    /// Hotspots kept per entry; `0` disables them (`hotspots`).
    pub hotspots: usize,
    /// Hotspots below this share of the total error are dropped (`hotspot_min_share`).
    pub hotspot_min_share: f64,
    /// Peak error at which any local hotspot fails an entry whose deciding
    /// metric passed (`hotspot_fail`); `None` (default) leaves it to the metric.
    pub hotspot_fail: Option<f64>,
    /// Whether a run that compared no pair is accepted (`allow_empty`,
    /// `--allow-empty`). Off by default: nothing compared is not a pass.
    pub allow_empty: bool,
    /// Whether a capture with NaN or infinite samples is an error
    /// (`fail_on_nonfinite`, default true).
    pub fail_on_nonfinite: bool,
    /// Diagnostics engine settings (`[diagnostics]`).
    pub diagnostics: crate::diagnostics::DiagnosticsConfig,
    /// The confidence gate for answers recorded by `saccade decide` (`[decisions]`).
    pub decisions: crate::decision::DecisionsConfig,
    /// Allowed external targets for capture symlinks in `serve`.
    pub symlink_targets: Vec<std::path::PathBuf>,
    /// Storage deadline for `serve`, in milliseconds.
    pub fs_timeout_ms: u64,
}

impl Default for RunConfig {
    fn default() -> Self {
        Self {
            default_threshold: 0.01,
            default_metric: Metric::Mean,
            pixels_per_degree: crate::compare::DEFAULT_PIXELS_PER_DEGREE,
            fail_on_new: false,
            ignore: Vec::new(),
            entries: Vec::new(),
            record_absolute_paths: false,
            overrides: Vec::new(),
            buffers: Vec::new(),
            regions: Vec::new(),
            masks: Vec::new(),
            config_dir: None,
            mode: Mode::default(),
            labels: Labels::default(),
            hdr: crate::hdr::HdrConfig::default(),
            meta: crate::meta::MetaOptions::default(),
            perf: crate::perf::PerfOptions::default(),
            hotspot_threshold: crate::hotspots::DEFAULT_HOTSPOT_THRESHOLD,
            hotspots: crate::hotspots::DEFAULT_HOTSPOTS,
            hotspot_min_share: crate::hotspots::DEFAULT_HOTSPOT_MIN_SHARE,
            hotspot_fail: None,
            allow_empty: false,
            fail_on_nonfinite: true,
            diagnostics: crate::diagnostics::DiagnosticsConfig::default(),
            decisions: crate::decision::DecisionsConfig::default(),
            symlink_targets: Vec::new(),
            fs_timeout_ms: 3000,
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FileConfig {
    #[serde(default)]
    symlink_targets: Vec<std::path::PathBuf>,
    fs_timeout_ms: Option<u64>,
    threshold: Option<f64>,
    metric: Option<Metric>,
    fail_on_new: Option<bool>,
    ppd: Option<f32>,
    meta_name: Option<String>,
    perf_name: Option<String>,
    perf_noise: Option<FilePerfNoise>,
    perf_noise_file: Option<std::path::PathBuf>,
    perf_noise_k: Option<f64>,
    perf_resolution_ms: Option<f64>,
    perf_resolution_ticks: Option<u32>,
    perf_min_delta_ms: Option<f64>,
    perf_min_delta_pct: Option<f64>,
    require_matching_meta: Option<bool>,
    hotspot_threshold: Option<f32>,
    hotspots: Option<usize>,
    hotspot_min_share: Option<f64>,
    hotspot_fail: Option<f64>,
    allow_empty: Option<bool>,
    fail_on_nonfinite: Option<bool>,
    #[serde(default)]
    ignore: Vec<String>,
    #[serde(default, rename = "override")]
    overrides: Vec<FileOverride>,
    #[serde(default, rename = "buffer")]
    buffers: Vec<crate::buffer::BufferSpec>,
    #[serde(default, rename = "region")]
    regions: Vec<crate::regions::RegionSpec>,
    #[serde(default, rename = "mask")]
    masks: Vec<crate::regions::MaskSpec>,
    hdr: Option<FileHdr>,
    diagnostics: Option<FileDiagnostics>,
    decisions: Option<crate::decision::DecisionsConfig>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum FilePerfNoise {
    Floor(crate::perf::PerfNoise),
    File(std::path::PathBuf),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FileDiagnostics {
    enabled: Option<bool>,
    shift_detection: Option<bool>,
    shift_min_px: Option<f64>,
    shift_min_confidence: Option<f64>,
    noise_max_flip: Option<f64>,
    explained_min: Option<f64>,
    partial_min: Option<f64>,
    perf_keys: Option<Vec<String>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FileHdr {
    tonemapper: Option<crate::hdr::Tonemapper>,
    start_exposure: Option<f32>,
    stop_exposure: Option<f32>,
    num_exposures: Option<u32>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FileOverride {
    glob: String,
    threshold: Option<f64>,
    metric: Option<Metric>,
}

pub(crate) fn compile_glob(pattern: &str) -> Result<globset::GlobMatcher> {
    GlobBuilder::new(pattern)
        .literal_separator(true)
        .case_insensitive(true)
        .build()
        .map(|g: Glob| g.compile_matcher())
        .map_err(|e| Error::Config(format!("invalid glob {pattern:?}: {e}")))
}

impl RunConfig {
    /// Parses a `saccade.toml` file (see `docs/design.md`). Unknown keys are errors.
    pub fn from_toml_file(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path).map_err(|source| Error::Io {
            context: format!("reading config {}", path.display()),
            source,
        })?;
        let mut cfg = Self::from_toml_str(&text).map_err(|e| match e {
            Error::Config(msg) => Error::Config(format!("{}: {msg}", path.display())),
            other => other,
        })?;
        cfg.config_dir = Some(
            path.parent()
                .filter(|p| !p.as_os_str().is_empty())
                .map_or_else(|| Path::new(".").to_path_buf(), Path::to_path_buf),
        );
        if let (Some(dir), Some(noise)) = (&cfg.config_dir, cfg.perf.noise.as_mut())
            && noise.is_relative()
        {
            *noise = dir.join(&*noise);
        }
        if let Some(dir) = &cfg.config_dir {
            for target in &mut cfg.symlink_targets {
                if target.is_relative() {
                    *target = dir.join(&*target);
                }
            }
        }
        if let (Some(dir), Some(cal)) = (&cfg.config_dir, cfg.decisions.calibration.as_mut())
            && cal.is_relative()
        {
            *cal = dir.join(&*cal);
        }
        Ok(cfg)
    }

    /// Parses `saccade.toml` contents.
    pub fn from_toml_str(text: &str) -> Result<Self> {
        let file: FileConfig = toml::from_str(text).map_err(|e| Error::Config(e.to_string()))?;
        let mut cfg = Self::default();
        cfg.perf.name = file.perf_name.unwrap_or(cfg.perf.name);
        cfg.perf.noise = file.perf_noise_file;
        match file.perf_noise {
            Some(FilePerfNoise::Floor(f)) => cfg.perf.floor = Some(f),
            Some(FilePerfNoise::File(p)) => {
                if cfg.perf.noise.is_some() {
                    return Err(Error::Config(
                        "use perf_noise or perf_noise_file, not both file keys".into(),
                    ));
                }
                cfg.perf.noise = Some(p);
            }
            None => {}
        }
        cfg.perf.k = file.perf_noise_k.unwrap_or(cfg.perf.k);
        cfg.perf.resolution_ms = file.perf_resolution_ms;
        cfg.perf.resolution_ticks = file.perf_resolution_ticks;
        cfg.perf.min_delta_ms = file.perf_min_delta_ms;
        cfg.perf.min_delta_pct = file.perf_min_delta_pct;
        if let Some(v) = file.threshold {
            cfg.default_threshold = v;
        }
        if let Some(v) = file.metric {
            cfg.default_metric = v;
        }
        if let Some(v) = file.fail_on_new {
            cfg.fail_on_new = v;
        }
        if let Some(v) = file.ppd {
            cfg.pixels_per_degree = v;
        }
        if let Some(v) = file.require_matching_meta {
            cfg.meta.required = v;
        }
        if let Some(v) = file.meta_name {
            cfg.meta.name = v;
        }
        if let Some(v) = file.hotspot_threshold {
            cfg.hotspot_threshold = v;
        }
        if let Some(v) = file.hotspots {
            cfg.hotspots = v;
        }
        if let Some(v) = file.hotspot_min_share {
            cfg.hotspot_min_share = v;
        }
        cfg.hotspot_fail = file.hotspot_fail;
        if let Some(v) = file.allow_empty {
            cfg.allow_empty = v;
        }
        if let Some(v) = file.fail_on_nonfinite {
            cfg.fail_on_nonfinite = v;
        }
        cfg.ignore = file.ignore;
        cfg.overrides = file
            .overrides
            .into_iter()
            .map(|o| Override {
                glob: o.glob,
                threshold: o.threshold,
                metric: o.metric,
            })
            .collect();
        cfg.regions = file.regions;
        cfg.buffers = file.buffers;
        cfg.masks = file.masks;
        if let Some(d) = file.decisions {
            cfg.decisions = d;
        }
        if let Some(d) = file.diagnostics {
            let t = &mut cfg.diagnostics;
            t.enabled = d.enabled.unwrap_or(t.enabled);
            t.shift_detection = d.shift_detection.unwrap_or(t.shift_detection);
            t.shift_min_px = d.shift_min_px.unwrap_or(t.shift_min_px);
            t.shift_min_confidence = d.shift_min_confidence.unwrap_or(t.shift_min_confidence);
            t.noise_max_flip = d.noise_max_flip.unwrap_or(t.noise_max_flip);
            t.explained_min = d.explained_min.unwrap_or(t.explained_min);
            t.partial_min = d.partial_min.unwrap_or(t.partial_min);
            if let Some(keys) = d.perf_keys {
                t.perf_keys = keys;
            }
        }
        if let Some(h) = file.hdr {
            if let Some(t) = h.tonemapper {
                cfg.hdr.tonemapper = t;
            }
            cfg.hdr.start_exposure = h.start_exposure;
            cfg.hdr.stop_exposure = h.stop_exposure;
            cfg.hdr.num_exposures = h.num_exposures;
        }
        cfg.symlink_targets = file.symlink_targets;
        cfg.fs_timeout_ms = file.fs_timeout_ms.unwrap_or(cfg.fs_timeout_ms);
        if cfg.fs_timeout_ms == 0 {
            return Err(Error::Config("fs_timeout_ms must be positive".into()));
        }
        cfg.validate()?;
        Ok(cfg)
    }

    /// Checks that every glob compiles, thresholds are finite and pixels per
    /// degree is finite and positive.
    pub fn validate(&self) -> Result<()> {
        if !self.default_threshold.is_finite() {
            return Err(Error::Config("threshold must be finite".into()));
        }
        crate::compare::check_ppd(self.pixels_per_degree)?;
        if !self.hotspot_threshold.is_finite() || !(0.0..1.0).contains(&self.hotspot_threshold) {
            return Err(Error::Config(format!(
                "hotspot_threshold must be in [0, 1), got {}",
                self.hotspot_threshold
            )));
        }
        if !self.hotspot_min_share.is_finite() || !(0.0..=1.0).contains(&self.hotspot_min_share) {
            return Err(Error::Config(format!(
                "hotspot_min_share must be in [0, 1], got {}",
                self.hotspot_min_share
            )));
        }
        if let Some(hf) = self.hotspot_fail {
            if !hf.is_finite() || hf <= 0.0 || hf > 1.0 {
                return Err(Error::Config(format!(
                    "hotspot_fail must be in (0, 1], got {hf}"
                )));
            }
            if hf < f64::from(self.hotspot_threshold) {
                return Err(Error::Config(format!(
                    "hotspot_fail ({hf}) must not be below hotspot_threshold ({})",
                    self.hotspot_threshold
                )));
            }
        }
        self.perf.validate()?;
        self.hdr.validate()?;
        self.diagnostics.validate()?;
        self.meta.checker()?;
        for g in self.ignore.iter().chain(&self.entries) {
            compile_glob(g)?;
        }
        for o in &self.overrides {
            compile_glob(&o.glob)?;
            if o.threshold.is_some_and(|t| !t.is_finite()) {
                return Err(Error::Config(format!(
                    "override {:?}: threshold must be finite",
                    o.glob
                )));
            }
        }
        crate::regions::validate(&self.regions, &self.masks)?;
        for buffer in &self.buffers {
            buffer.validate()?;
        }
        Ok(())
    }

    /// Effective `(metric, threshold)` for an image name: the first matching
    /// override wins, field by field over the defaults.
    pub fn effective_for(&self, name: &str) -> (Metric, f64) {
        let mut metric = self.default_metric;
        let mut threshold = self.default_threshold;
        let hit = self
            .overrides
            .iter()
            .find(|o| compile_glob(&o.glob).is_ok_and(|m| m.is_match(name)));
        if let Some(o) = hit {
            if let Some(m) = o.metric {
                metric = m;
            }
            if let Some(t) = o.threshold {
                threshold = t;
            }
        }
        (metric, threshold)
    }

    /// Effective settings with configuration and per-image provenance.
    pub fn explain_settings(
        &self,
        file: Option<&Path>,
        reason: &str,
        name: Option<&str>,
    ) -> Result<serde_json::Value> {
        use serde_json::{Value, json};
        fn settings(c: &RunConfig) -> Value {
            json!({
                "threshold": c.default_threshold, "metric": c.default_metric,
                "ppd": c.pixels_per_degree, "fail_on_new": c.fail_on_new,
                "require_matching_meta": c.meta.required, "meta_name": c.meta.name,
                "meta_ignore": c.meta.ignore, "declare": c.meta.declared,
                "perf_name": c.perf.name, "perf_noise_k": c.perf.k,
                "perf_resolution_ms": c.perf.resolution_ms,
                "perf_resolution_ticks": c.perf.resolution_ticks,
                "perf_min_delta_ms": c.perf.min_delta_ms,
                "perf_min_delta_pct": c.perf.min_delta_pct,
                "perf_noise_file": c.perf.noise.as_ref().map(|p| crate::paths::cwd(p,false)),
                "perf_noise": c.perf.floor,
                "hotspot_threshold": c.hotspot_threshold, "hotspots": c.hotspots,
                "hotspot_min_share": c.hotspot_min_share, "hotspot_fail": c.hotspot_fail,
                "allow_empty": c.allow_empty, "fail_on_nonfinite": c.fail_on_nonfinite,
                "ignore": c.ignore,
                "hdr": {"tonemapper": c.hdr.tonemapper.name(), "start_exposure": c.hdr.start_exposure, "stop_exposure": c.hdr.stop_exposure, "num_exposures": c.hdr.num_exposures},
                "diagnostics": {"enabled": c.diagnostics.enabled, "shift_detection": c.diagnostics.shift_detection,
                    "shift_min_px": c.diagnostics.shift_min_px, "shift_min_confidence": c.diagnostics.shift_min_confidence,
                    "noise_max_flip": c.diagnostics.noise_max_flip, "explained_min": c.diagnostics.explained_min,
                    "partial_min": c.diagnostics.partial_min, "perf_keys": c.diagnostics.perf_keys},
                "decisions": {"auto_accept_min_prob": c.decisions.auto_accept_min_prob,
                    "allow_sources": c.decisions.allow_sources, "gate_on": c.decisions.gate_on, "calibration": c.decisions.calibration.as_ref().map(|p| crate::paths::cwd(p, false))}
            })
        }
        let raw: Value = match file {
            Some(p) => {
                let text = std::fs::read_to_string(p).map_err(|source| Error::Io {
                    context: format!("reading config {}", p.display()),
                    source,
                })?;
                let parsed: toml::Value =
                    toml::from_str(&text).map_err(|e| Error::Config(e.to_string()))?;
                serde_json::to_value(parsed)?
            }
            None => json!({}),
        };
        let origin = file
            .map(|p| crate::paths::cwd(p, false))
            .unwrap_or_else(|| "built-in default".into());
        fn sourced(value: &Value, raw: &Value, origin: &str, key: &str) -> Value {
            if let Value::Object(m) = value {
                let mut result = serde_json::Map::new();
                for (k, v) in m {
                    result.insert(
                        k.clone(),
                        sourced(v, &raw[k], origin, &format!("{key}.{k}")),
                    );
                }
                Value::Object(result)
            } else {
                json!({"value": value, "source": if raw.is_null() { "built-in default".into() } else { format!("{origin}:{key}") }})
            }
        }
        let defaults = settings(&Self::default());
        let effective = settings(self);
        let mut values = serde_json::Map::new();
        if let Value::Object(m) = &effective {
            for (k, v) in m {
                values.insert(k.clone(), sourced(v, &raw[k], &origin, k));
            }
        }
        let image = if let Some(name) = name {
            let matched = self
                .overrides
                .iter()
                .enumerate()
                .find(|(_, o)| compile_glob(&o.glob).is_ok_and(|g| g.is_match(name)));
            let buffer = self
                .buffers
                .iter()
                .enumerate()
                .find(|(_, b)| compile_glob(&b.glob).is_ok_and(|g| g.is_match(name)));
            let (metric, threshold) = buffer.map_or_else(
                || self.effective_for(name),
                |(_, b)| (b.metric, b.threshold()),
            );
            let source = |field: &str, overridden: bool| {
                if let Some((i, _)) = buffer {
                    if raw["buffer"][i].get(field).is_some() {
                        format!("{origin}:buffer[{i}].{field}")
                    } else {
                        format!("built-in buffer default ({origin}:buffer[{i}])")
                    }
                } else if overridden {
                    format!(
                        "{origin}:override[{}].{field}",
                        matched.map_or(0, |(i, _)| i)
                    )
                } else if raw.get(field).is_some() {
                    format!("{origin}:{field}")
                } else {
                    "built-in default".into()
                }
            };
            let applies =
                |g: Option<&str>| g.is_none_or(|g| compile_glob(g).is_ok_and(|m| m.is_match(name)));
            let regions: Vec<Value> = self.regions.iter().enumerate().filter(|(_,r)| applies(r.glob.as_deref())).map(|(i,r)| json!({
                "name": r.name, "glob": r.glob, "rect": r.rect, "threshold": r.threshold,
                "metric": r.metric.unwrap_or(metric), "source": format!("{origin}:region[{i}]"),
                "metric_source": if r.metric.is_some() { format!("{origin}:region[{i}].metric") } else { source("metric", matched.is_some_and(|(_,o)| o.metric.is_some())) }
            })).collect();
            let masks: Vec<Value> = self.masks.iter().enumerate().filter(|(_,m)| applies(m.glob.as_deref())).map(|(i,m)| json!({"glob": m.glob, "rect": m.rect, "image": m.image, "source": format!("{origin}:mask[{i}]")})).collect();
            json!({"name": name,
                "matching_override": matched.map(|(i,o)| json!({"glob": o.glob,"threshold": o.threshold,"metric":o.metric,"source":format!("{origin}:override[{i}]")})),
                "metric": {"value":metric,"source":source("metric",matched.is_some_and(|(_,o)| o.metric.is_some()))},
                "threshold": {"value":threshold,"source":source("threshold",matched.is_some_and(|(_,o)| o.threshold.is_some()))},
                "regions": regions, "masks": masks, "regions_and_masks_applied": buffer.is_none(),
                "buffer": raw["buffer"].as_array().and_then(|a| a.iter().enumerate().find(|(_,b)| b["glob"].as_str().is_some_and(|g| applies(Some(g))))).map(|(i,b)| json!({"value":b,"source":format!("{origin}:buffer[{i}]")})),
                "ignored": self.ignore.iter().any(|g| compile_glob(g).is_ok_and(|m| m.is_match(name)))
            })
        } else {
            Value::Null
        };
        Ok(
            json!({"config_file":file.map(|p| crate::paths::cwd(p,false)),"reason":reason,"defaults":defaults,"settings":values,
            "override":raw.get("override").cloned().unwrap_or_else(||json!([])),
            "regions":raw.get("region").cloned().unwrap_or_else(||json!([])),
            "masks":raw.get("mask").cloned().unwrap_or_else(||json!([])),
            "buffers":raw.get("buffer").cloned().unwrap_or_else(||json!([])),"image":image}),
        )
    }

    /// First numerical-buffer rule matching this relative image name.
    pub fn buffer_for(&self, name: &str) -> Option<&crate::buffer::BufferSpec> {
        self.buffers
            .iter()
            .find(|b| compile_glob(&b.glob).is_ok_and(|m| m.is_match(name)))
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn first_matching_override_wins_field_by_field() {
        let cfg = RunConfig::from_toml_str(
            r#"
            threshold = 0.02
            metric = "mean"
            [[override]]
            glob = "terrain/**"
            threshold = 0.03
            [[override]]
            glob = "terrain/**"
            threshold = 0.5
            metric = "max"
            [[override]]
            glob = "**/*.png"
            metric = "p95"
            "#,
        )
        .expect("parse");
        // First match supplies the threshold only; metric stays the default.
        assert_eq!(cfg.effective_for("terrain/a.png"), (Metric::Mean, 0.03));
        assert_eq!(cfg.effective_for("ui/a.png"), (Metric::P95, 0.02));
        assert_eq!(cfg.effective_for("ui/a.jpg"), (Metric::Mean, 0.02));
    }

    #[test]
    fn globs_ignore_case() {
        let cfg = RunConfig::from_toml_str(
            "ignore = [\"**/*.png\"]\n[[override]]\nglob = \"UI/*.PNG\"\nthreshold = 0.5\n",
        )
        .expect("parse");
        assert_eq!(cfg.effective_for("ui/a.png").1, 0.5);
        assert!(compile_glob("**/*.png").expect("glob").is_match("x/Y.PNG"));
    }
}
