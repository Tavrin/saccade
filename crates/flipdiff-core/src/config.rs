//! Run configuration and the `flipdiff.toml` file format.

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
    /// The confidence gate for answers recorded by `flipdiff decide` (`[decisions]`).
    pub decisions: crate::decision::DecisionsConfig,
}

impl Default for RunConfig {
    fn default() -> Self {
        Self {
            default_threshold: 0.01,
            default_metric: Metric::Mean,
            pixels_per_degree: nv_flip::DEFAULT_PIXELS_PER_DEGREE,
            fail_on_new: false,
            ignore: Vec::new(),
            overrides: Vec::new(),
            buffers: Vec::new(),
            regions: Vec::new(),
            masks: Vec::new(),
            config_dir: None,
            mode: Mode::default(),
            labels: Labels::default(),
            hdr: crate::hdr::HdrConfig::default(),
            meta: crate::meta::MetaOptions::default(),
            hotspot_threshold: crate::hotspots::DEFAULT_HOTSPOT_THRESHOLD,
            hotspots: crate::hotspots::DEFAULT_HOTSPOTS,
            hotspot_min_share: crate::hotspots::DEFAULT_HOTSPOT_MIN_SHARE,
            hotspot_fail: None,
            allow_empty: false,
            fail_on_nonfinite: true,
            diagnostics: crate::diagnostics::DiagnosticsConfig::default(),
            decisions: crate::decision::DecisionsConfig::default(),
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FileConfig {
    threshold: Option<f64>,
    metric: Option<Metric>,
    fail_on_new: Option<bool>,
    ppd: Option<f32>,
    meta_name: Option<String>,
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
    /// Parses a `flipdiff.toml` file (see `docs/design.md`). Unknown keys are errors.
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
        if let (Some(dir), Some(cal)) = (&cfg.config_dir, cfg.decisions.calibration.as_mut()) {
            if cal.is_relative() {
                *cal = dir.join(&*cal);
            }
        }
        Ok(cfg)
    }

    /// Parses `flipdiff.toml` contents.
    pub fn from_toml_str(text: &str) -> Result<Self> {
        let file: FileConfig = toml::from_str(text).map_err(|e| Error::Config(e.to_string()))?;
        let mut cfg = Self::default();
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
        self.hdr.validate()?;
        self.diagnostics.validate()?;
        self.meta.checker()?;
        for g in &self.ignore {
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
